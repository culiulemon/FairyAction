use crate::params::{ActionDef, ActionResult, get_string, parse_action_params};
use crate::registry::Registry;
use fa_bridge::message::{BridgeMessage, BridgeMessageType};
use fa_fap::invoke::{self, SpecialVars};
use fa_fap::manifest::Manifest;
use fa_fap::package;
use fa_fap::parser;
use fa_fap::process;
use fa_fap::platform;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

pub struct FapManager {
    install_dir: PathBuf,
    manifests: RwLock<HashMap<String, Manifest>>,
    config: fa_config::FapConfig,
}

impl FapManager {
    pub fn new(config: &fa_config::FapConfig) -> Self {
        let install_dir = PathBuf::from(config.resolved_install_dir());
        Self {
            install_dir,
            manifests: RwLock::new(HashMap::new()),
            config: config.clone(),
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

        let channel = message.channel.as_ref().map(|s| s.as_str());
        let action_name = message.action.as_ref().map(|s| s.as_str());

        let manifests = self.manifests.read().await;
        let manifest = match manifests.get(module) {
            Some(m) => m,
            None => return ActionResult::error(format!("FAP package not found: {}", module)),
        };

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

        let special_vars = SpecialVars {
            temp_dir: self.config.resolved_temp_dir(),
            package_dir: self.install_dir.join(module).to_string_lossy().to_string(),
            host_data_dir: self.config.host_data_dir.clone(),
        };

        let args = match invoke::render_invoke(invoke_config, &message.payload, &special_vars) {
            Ok(a) => a,
            Err(e) => return ActionResult::error(format!("invoke render error: {}", e)),
        };

        let env = invoke_config.env.as_ref();
        let timeout = invoke_config.timeout.or(Some(self.config.default_timeout));

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
}
