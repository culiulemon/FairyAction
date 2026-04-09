use crate::params::{ActionDef, ActionResult, get_string, parse_action_params};
use crate::registry::Registry;
use fa_bridge::message::{BridgeMessage, BridgeMessageType};
use fa_fap::invoke::{self, SpecialVars};
use fa_fap::manifest::{Manifest, PackageMode};
use fa_fap::package;
use fa_fap::parser;
use fa_fap::platform;
use fa_fap::process;
use fa_fap::process_pool::ProcessPool;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex as AsyncMutex};
use tracing::{info, warn};

pub struct FapManager {
    install_dir: PathBuf,
    manifests: RwLock<HashMap<String, Manifest>>,
    config: RwLock<fa_config::FapConfig>,
    process_pool: AsyncMutex<ProcessPool>,
}

impl FapManager {
    pub fn new(config: &fa_config::FapConfig) -> Self {
        let install_dir = PathBuf::from(config.resolved_install_dir());
        Self {
            install_dir,
            manifests: RwLock::new(HashMap::new()),
            config: RwLock::new(config.clone()),
            process_pool: AsyncMutex::new(ProcessPool::new()),
        }
    }

    pub async fn refresh_manifests(&self) -> anyhow::Result<()> {
        let packages = package::list_packages(&self.install_dir).await?;
        let mut manifests = self.manifests.write().await;
        manifests.clear();
        for pkg in packages {
            match package::inspect_package(&pkg.package, &self.install_dir).await {
                Ok(manifest) => {
                    info!(package = %pkg.package, "Loaded FAP manifest");
                    manifests.insert(pkg.package.clone(), manifest);
                }
                Err(e) => {
                    warn!(package = %pkg.package, error = %e, "Failed to load manifest");
                }
            }
        }
        Ok(())
    }

    pub async fn get_capabilities_description(&self) -> String {
        let manifests = self.manifests.read().await;
        let mut desc = String::new();
        for (pkg_name, manifest) in manifests.iter() {
            desc.push_str(&format!("\n## {} ({})\n", manifest.name, pkg_name));
            for (_domain_name, domains) in &manifest.capabilities {
                for domain in domains {
                    desc.push_str(&format!("### {}\n", domain.名称));
                    for action in &domain.动作 {
                        desc.push_str(&format!("- {}: ", action.名称));
                        let params: Vec<String> = action.参数.iter().map(|(k, v)| {
                            let req = v.必填.unwrap_or(false);
                            format!("{}({}:{})", k, v.param_type, if req { "必填" } else { "可选" })
                        }).collect();
                        desc.push_str(&params.join(", "));
                        desc.push('\n');
                    }
                }
            }
        }
        desc
    }

    pub async fn handle_bridge_call(&self, message: &BridgeMessage) -> ActionResult {
        let module = match &message.module {
            Some(m) => m,
            None => return ActionResult::error("missing module in bridge call"),
        };

        let manifests = self.manifests.read().await;
        let manifest = match manifests.get(module) {
            Some(m) => m,
            None => return ActionResult::error(format!("FAP package not found: {}", module)),
        };

        if manifest.permissions.is_empty() {
            warn!(
                package = %module,
                "FAP package has no permissions declared"
            );
        } else {
            info!(
                package = %module,
                permissions = ?manifest.permissions,
                "FAP package permissions"
            );
        }

        match manifest.mode {
            PackageMode::Manifest => {
                self.handle_manifest_call(manifest, module, message).await
            }
            PackageMode::Sdk => {
                self.handle_sdk_call(manifest, module, message).await
            }
        }
    }

    async fn handle_manifest_call(&self, manifest: &Manifest, module: &str, message: &BridgeMessage) -> ActionResult {
        let channel = message.channel.as_ref().map(|s| s.as_str());
        let action_name = message.action.as_ref().map(|s| s.as_str());

        let platform_str = platform::detect_platform();
        let binary = match manifest.entry.get(&platform_str) {
            Some(e) => self.install_dir.join(module).join(e),
            None => return ActionResult::error(format!("no binary for platform: {}", platform_str)),
        };

        let mut found_action: Option<&fa_fap::manifest::Action> = None;
        for (_, domains) in &manifest.capabilities {
            for domain in domains {
                if channel.is_some() && domain.名称 != channel.unwrap() {
                    continue;
                }
                for action in &domain.动作 {
                    if action_name.is_some() && action.名称 != action_name.unwrap() {
                        continue;
                    }
                    found_action = Some(action);
                    break;
                }
                if found_action.is_some() { break; }
            }
            if found_action.is_some() { break; }
        }

        let action = match found_action {
            Some(a) => a,
            None => return ActionResult::error(format!(
                "action not found: {} / {:?}",
                channel.unwrap_or("?"),
                action_name.unwrap_or("?")
            )),
        };

        let invoke_config = match &action.invoke {
            Some(i) => i,
            None => return ActionResult::error("action has no invoke config"),
        };

        let config = self.config.read().await;
        let special_vars = SpecialVars {
            temp_dir: config.resolved_temp_dir(),
            package_dir: self.install_dir.join(module).to_string_lossy().to_string(),
            host_data_dir: config.host_data_dir.clone(),
        };
        let default_timeout = config.default_timeout;
        drop(config);

        let args = match invoke::render_invoke(invoke_config, &message.payload, &special_vars) {
            Ok(a) => a,
            Err(e) => return ActionResult::error(format!("invoke render error: {}", e)),
        };

        let env = invoke_config.env.as_ref();
        let timeout = invoke_config.timeout.or(Some(default_timeout));

        let result = match process::execute_process(
            &binary,
            &args,
            env,
            None,
            timeout,
        ).await {
            Ok(r) => r,
            Err(e) => return ActionResult::error(format!("process error: {}", e)),
        };

        let exit_status = invoke_config.exit_code.as_ref()
            .and_then(|m| m.get(&result.exit_code.to_string()))
            .map(|s| s.as_str())
            .unwrap_or(if result.exit_code == 0 { "success" } else { "error" });

        if exit_status == "error" {
            return ActionResult::error(format!(
                "process exited with code {}: {}",
                result.exit_code,
                result.stderr.chars().take(500).collect::<String>()
            ));
        }

        let output_text = match &invoke_config.output {
            Some(output_config) => {
                let raw = if output_config.source == "stderr" {
                    &result.stderr
                } else {
                    &result.stdout
                };
                match parser::parse_output(&output_config.parser, raw, output_config.pattern.as_deref()) {
                    Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| raw.clone()),
                    Err(e) => {
                        warn!(error = %e, "parser error, returning raw output");
                        raw.clone()
                    }
                }
            }
            None => result.stdout,
        };

        ActionResult::success(output_text)
    }

    async fn handle_sdk_call(&self, manifest: &Manifest, module: &str, message: &BridgeMessage) -> ActionResult {
        let channel = message.channel.as_ref().map(|s| s.as_str()).unwrap_or("");
        let action_name = message.action.as_ref().map(|s| s.as_str()).unwrap_or("");

        let lifecycle = manifest.lifecycle.as_ref();
        let use_persistent = lifecycle.map_or(false, |l| {
            matches!(l, fa_fap::manifest::Lifecycle::Persistent | fa_fap::manifest::Lifecycle::Both)
        });

        if use_persistent {
            let platform_str = platform::detect_platform();
            let binary = match manifest.entry.get(&platform_str) {
                Some(e) => self.install_dir.join(module).join(e),
                None => return ActionResult::error(format!("no binary for platform: {}", platform_str)),
            };

            let mut pool = self.process_pool.lock().await;

            if let Err(e) = pool.get_or_spawn(module, &binary).await {
                return ActionResult::error(format!("failed to spawn persistent process: {}", e));
            }

            match pool.send_call(module, channel, action_name, &message.payload).await {
                Ok(result) => {
                    if result.success {
                        ActionResult::success(serde_json::to_string_pretty(&result.payload).unwrap_or_default())
                    } else {
                        ActionResult::error(serde_json::to_string_pretty(&result.payload).unwrap_or_default())
                    }
                }
                Err(e) => ActionResult::error(format!("persistent call error: {}", e)),
            }
        } else {
            let platform_str = platform::detect_platform();
            let binary = match manifest.entry.get(&platform_str) {
                Some(e) => self.install_dir.join(module).join(e),
                None => return ActionResult::error(format!("no binary for platform: {}", platform_str)),
            };

            let mut args = vec![action_name.to_string()];
            if let Value::Object(map) = &message.payload {
                for (key, value) in map {
                    args.push(format!("--{}", key));
                    match value {
                        Value::String(s) => args.push(s.clone()),
                        Value::Number(n) => args.push(n.to_string()),
                        Value::Bool(b) => {
                            if !b {
                                args.pop();
                                args.push(format!("--no-{}", key));
                            }
                        }
                        _ => args.push(value.to_string()),
                    }
                }
            }

            let config = self.config.read().await;
            let timeout = Some(config.default_timeout);
            drop(config);

            match process::execute_process(&binary, &args, None, None, timeout).await {
                Ok(result) => {
                    if result.exit_code == 0 {
                        ActionResult::success(result.stdout)
                    } else {
                        ActionResult::error(result.stderr.chars().take(500).collect::<String>())
                    }
                }
                Err(e) => ActionResult::error(format!("process error: {}", e)),
            }
        }
    }

    pub async fn handle_hello(&self, module: Option<&str>) -> ActionResult {
        let manifests = self.manifests.read().await;
        let mut result = serde_json::Map::new();

        if let Some(module_name) = module {
            if let Some(manifest) = manifests.get(module_name) {
                result.insert("module".to_string(), Value::String(module_name.to_string()));
                result.insert("capabilities".to_string(), manifest_to_capabilities_json(manifest));
            } else {
                return ActionResult::error(format!("module not found: {}", module_name));
            }
        } else {
            let modules: serde_json::Map<String, Value> = manifests.iter()
                .map(|(name, manifest)| {
                    (name.clone(), manifest_to_capabilities_json(manifest))
                })
                .collect();
            result.insert("modules".to_string(), Value::Object(modules));
        }

        ActionResult::success(serde_json::to_string_pretty(&result).unwrap())
    }

    pub async fn handle_configure(&self, payload: &Value) -> ActionResult {
        let mut config = self.config.write().await;
        let mut updated = Vec::new();

        if let Value::Object(map) = payload {
            for (key, value) in map {
                let str_value = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    other => other.to_string(),
                };

                match key.as_str() {
                    "fap.install_dir" => { config.install_dir = Some(str_value); updated.push(key.clone()); }
                    "fap.temp_dir" => { config.temp_dir = Some(str_value); updated.push(key.clone()); }
                    "fap.host_data_dir" => { config.host_data_dir = Some(str_value); updated.push(key.clone()); }
                    "fap.default_timeout" => {
                        if let Ok(t) = str_value.parse::<u32>() { config.default_timeout = t; updated.push(key.clone()); }
                    }
                    "fap.max_concurrent" => {
                        if let Ok(c) = str_value.parse::<u32>() { config.max_concurrent = Some(c); updated.push(key.clone()); }
                    }
                    _ => {}
                }
            }
        }

        let install_dir_changed = updated.iter().any(|k| k == "fap.install_dir");
        drop(config);

        if install_dir_changed {
            if let Err(e) = self.refresh_manifests().await {
                return ActionResult::error(format!("configure applied but refresh failed: {}", e));
            }
        }

        ActionResult::success(serde_json::to_string_pretty(&serde_json::json!({
            "applied": true,
            "updated": updated
        })).unwrap())
    }
}

fn manifest_to_capabilities_json(manifest: &Manifest) -> Value {
    let domains: Vec<Value> = manifest.capabilities.iter()
        .flat_map(|(_, domain_list)| domain_list.iter())
        .map(|domain| {
            let actions: Vec<Value> = domain.动作.iter().map(|action| {
                let params: serde_json::Map<String, Value> = action.参数.iter().map(|(k, v)| {
                    (k.clone(), serde_json::json!({
                        "类型": v.param_type,
                        "必填": v.必填.unwrap_or(false),
                        "描述": v.描述.as_ref().unwrap_or(&"".to_string())
                    }))
                }).collect();
                serde_json::json!({
                    "名称": action.名称,
                    "参数": params
                })
            }).collect();
            serde_json::json!({
                "名称": domain.名称,
                "动作": actions
            })
        }).collect();
    serde_json::json!({"能力域": domains})
}

pub async fn register_fap_actions(registry: &Registry, config: &fa_config::FapConfig) {
    let manager = Arc::new(FapManager::new(config));

    match manager.refresh_manifests().await {
        Ok(_) => {
            let count = {
                let m = manager.manifests.read().await;
                m.len()
            };
            info!(count = count, "FAP packages loaded");
        }
        Err(e) => {
            warn!(error = %e, "Failed to refresh FAP manifests");
        }
    }

    let mgr = manager.clone();
    registry.register(
        ActionDef::new("fap_call", "调用 FAP 包中的动作")
            .param("module", crate::params::ParamType::String, "FAP 包名")
            .optional_param("channel", crate::params::ParamType::String, "能力域名称", Value::Null)
            .optional_param("action", crate::params::ParamType::String, "动作名称", Value::Null)
            .optional_param("params", crate::params::ParamType::String, "JSON 参数", Value::String("{}".to_string())),
        move |_ctx, params| {
            let mgr = mgr.clone();
            async move {
                let p = parse_action_params(&params);
                let module = match get_string(&p, "module") {
                    Some(m) => m,
                    None => return ActionResult::error("missing parameter: module"),
                };
                let channel = get_string(&p, "channel");
                let action = get_string(&p, "action");
                let params_str = get_string(&p, "params").unwrap_or_else(|| "{}".to_string());
                let payload: Value = match serde_json::from_str(&params_str) {
                    Ok(v) => v,
                    Err(e) => return ActionResult::error(format!("invalid JSON params: {}", e)),
                };

                let message = BridgeMessage {
                    message_type: BridgeMessageType::Call,
                    module: Some(module),
                    channel,
                    action,
                    payload,
                };

                mgr.handle_bridge_call(&message).await
            }
        },
    ).await;

    let mgr = manager.clone();
    registry.register(
        ActionDef::new("fap_list", "列出已安装的 FAP 包及其能力")
            .optional_param("module", crate::params::ParamType::String, "指定查看的包名", Value::Null),
        move |_ctx, _params| {
            let mgr = mgr.clone();
            async move {
                let desc = mgr.get_capabilities_description().await;
                if desc.is_empty() {
                    ActionResult::success("No FAP packages installed".to_string())
                } else {
                    ActionResult::success(desc)
                }
            }
        },
    ).await;

    let mgr = manager.clone();
    registry.register(
        ActionDef::new("fap_hello", "查询 FAP 能力池")
            .optional_param("module", crate::params::ParamType::String, "指定查询的包名", Value::Null),
        move |_ctx, params| {
            let mgr = mgr.clone();
            async move {
                let p = parse_action_params(&params);
                let module = get_string(&p, "module");
                mgr.handle_hello(module.as_deref()).await
            }
        },
    ).await;

    let mgr = manager.clone();
    registry.register(
        ActionDef::new("fap_configure", "运行时配置注入")
            .optional_param("config", crate::params::ParamType::String, "JSON 配置", Value::String("{}".to_string())),
        move |_ctx, params| {
            let mgr = mgr.clone();
            async move {
                let p = parse_action_params(&params);
                let config_str = get_string(&p, "config").unwrap_or_else(|| "{}".to_string());
                let payload: Value = match serde_json::from_str(&config_str) {
                    Ok(v) => v,
                    Err(e) => return ActionResult::error(format!("invalid JSON config: {}", e)),
                };
                mgr.handle_configure(&payload).await
            }
        },
    ).await;
}
