use clap::{Parser, Subcommand};
use fa_browser::profile::BrowserProfile;
use fa_browser::session::BrowserSession;
use fa_config::Config;
use fa_dom::service::DomService;
use fa_tools::actions::ActionContext;
use fa_tools::registry::Registry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::Arc;
use tracing::{debug, error, info};

#[derive(Parser)]
#[command(name = "fairy-action")]
#[command(about = "FairyAction - Browser automation toolkit for AI agents")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true)]
    config: Option<String>,

    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(long, help = "Run in non-headless mode (show browser)")]
        show_browser: bool,
    },

    #[command(name = "list-actions")]
    ListActions,

    Tester,

    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    Show,
    Init,
    Set {
        key: String,
        value: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Request {
    #[serde(rename = "execute")]
    Execute {
        action: String,
        params: Value,
    },
    #[serde(rename = "get_state")]
    GetState,
    #[serde(rename = "get_dom")]
    GetDom {
        #[serde(default)]
        show_empty: bool,
    },
    #[serde(rename = "list_actions")]
    ListActions,
    #[serde(rename = "toggle_annotations")]
    ToggleAnnotations {
        #[serde(default)]
        show: Option<bool>,
    },
    #[serde(rename = "close")]
    Close,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum Response {
    #[serde(rename = "ok")]
    Ok {
        action: Option<String>,
        result: ActionResultData,
    },
    #[serde(rename = "state")]
    State {
        url: String,
        title: String,
        tabs: Vec<TabInfoData>,
        viewport: Option<ViewportData>,
        scroll: Option<ScrollData>,
    },
    #[serde(rename = "dom")]
    Dom {
        url: String,
        title: String,
        representation: String,
        element_count: usize,
    },
    #[serde(rename = "actions")]
    Actions {
        actions: Vec<ActionDefData>,
        schema: Value,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
    },
    #[serde(rename = "closed")]
    Closed,
}

#[derive(Debug, Serialize)]
struct ActionResultData {
    success: bool,
    output: Option<String>,
    error: Option<String>,
    extracted_content: Option<String>,
    is_done: bool,
    state_after: Option<StateAfterData>,
}

#[derive(Debug, Serialize)]
struct StateAfterData {
    url: Option<String>,
    title: Option<String>,
    tab_count: Option<usize>,
    new_tab_opened: Option<bool>,
    navigation_occurred: Option<bool>,
    screenshot: Option<String>,
}

#[derive(Debug, Serialize)]
struct TabInfoData {
    id: String,
    url: String,
    title: String,
    is_active: bool,
}

#[derive(Debug, Serialize)]
struct ViewportData {
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
struct ScrollData {
    x: f64,
    y: f64,
}

#[derive(Debug, Serialize)]
struct ActionDefData {
    name: String,
    description: String,
    params: Vec<ParamDefData>,
}

#[derive(Debug, Serialize)]
struct ParamDefData {
    name: String,
    param_type: String,
    description: String,
    required: bool,
    default: Option<Value>,
    enum_values: Option<Vec<String>>,
}

fn convert_action_defs(defs: &[fa_tools::params::ActionDef]) -> Vec<ActionDefData> {
    defs.iter().map(|d| {
        ActionDefData {
            name: d.name.clone(),
            description: d.description.clone(),
            params: d.params.iter().map(|p| {
                ParamDefData {
                    name: p.name.clone(),
                    param_type: p.param_type.to_json_schema_type().to_string(),
                    description: p.description.clone(),
                    required: p.required,
                    default: p.default.clone(),
                    enum_values: p.enum_values.clone(),
                }
            }).collect(),
        }
    }).collect()
}

fn action_result_to_data(r: fa_tools::params::ActionResult) -> ActionResultData {
    ActionResultData {
        success: r.success,
        output: r.output,
        error: r.error,
        extracted_content: r.extracted_content,
        is_done: r.is_done,
        state_after: r.state_after.map(|s| StateAfterData {
            url: s.url,
            title: s.title,
            tab_count: s.tab_count,
            new_tab_opened: s.new_tab_opened,
            navigation_occurred: s.navigation_occurred,
            screenshot: s.screenshot,
        }),
    }
}

fn write_response(resp: &Response) {
    let mut line = serde_json::to_string(resp).unwrap_or_else(|e| {
        serde_json::to_string(&Response::Error { message: format!("Serialization error: {}", e) }).unwrap()
    });
    line.push('\n');
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = out.write_all(line.as_bytes());
    let _ = out.flush();
}

async fn handle_request(req: Request, session: &Arc<BrowserSession>, registry: &Arc<Registry>) -> Response {
    match req {
        Request::Execute { action, params } => {
            debug!(action = %action, "Executing action via request");

            let url = session.get_url().await.unwrap_or_default();
            let title = session.get_title().await.unwrap_or_default();

            let ctx = ActionContext::new(session.clone(), url, title);

            match registry.execute(&action, params, ctx).await {
                Ok(result) => Response::Ok {
                    action: Some(action),
                    result: action_result_to_data(result),
                },
                Err(e) => Response::Error {
                    message: e.to_string(),
                },
            }
        }

        Request::GetState => {
            let url = session.get_url().await.unwrap_or_default();
            let title = session.get_title().await.unwrap_or_default();
            let tabs = session.get_tabs().await.unwrap_or_default();
            let tab_data: Vec<TabInfoData> = tabs.iter().map(|t| {
                TabInfoData {
                    id: t.id.clone(),
                    url: t.url.clone(),
                    title: t.title.clone(),
                    is_active: t.is_active,
                }
            }).collect();

            let page_info = session
                .evaluate_js(
                    r#"(function() { return { width: window.innerWidth, height: window.innerHeight, scrollX: window.scrollX, scrollY: window.scrollY }; })()"#,
                )
                .await
                .ok()
                .and_then(|v| {
                    let obj = &v["result"]["value"];
                    Some((
                        obj["width"].as_u64()? as u32,
                        obj["height"].as_u64()? as u32,
                        obj["scrollX"].as_f64().unwrap_or(0.0),
                        obj["scrollY"].as_f64().unwrap_or(0.0),
                    ))
                });

            let (viewport, scroll) = match page_info {
                Some((w, h, sx, sy)) => (
                    Some(ViewportData { width: w, height: h }),
                    Some(ScrollData { x: sx, y: sy }),
                ),
                None => (None, None),
            };

            Response::State {
                url,
                title,
                tabs: tab_data,
                viewport,
                scroll,
            }
        }

        Request::GetDom { show_empty } => {
            let url = session.get_url().await.unwrap_or_default();
            let title = session.get_title().await.unwrap_or_default();

            let result = if show_empty {
                DomService::get_dom_state_full(session).await
            } else {
                DomService::get_dom_state(session).await
            };
            match result {
                Ok(dom_state) => {
                    let element_count = dom_state.selector_map.len();
                    Response::Dom {
                        url,
                        title,
                        representation: dom_state.llm_representation,
                        element_count,
                    }
                }
                Err(e) => Response::Error {
                    message: format!("Failed to get DOM: {}", e),
                },
            }
        }

        Request::ListActions => {
            let defs = registry.action_definitions().await;
            let schema = registry.get_action_schema().await;
            Response::Actions {
                actions: convert_action_defs(&defs),
                schema,
            }
        }

        Request::ToggleAnnotations { show } => {
            let result = match show {
                Some(true) => DomService::show_annotations(session).await,
                Some(false) => DomService::hide_annotations(session).await,
                None => DomService::toggle_annotations(session).await,
            };
            match result {
                Ok(visible) => Response::Ok {
                    action: None,
                    result: ActionResultData {
                        success: true,
                        output: Some(if visible { "Annotations shown".to_string() } else { "Annotations hidden".to_string() }),
                        error: None,
                        extracted_content: None,
                        is_done: false,
                        state_after: None,
                    },
                },
                Err(e) => Response::Error {
                    message: format!("Toggle annotations failed: {}", e),
                },
            }
        }

        Request::Close => {
            Response::Closed
        }
    }
}

async fn run_interactive(session: Arc<BrowserSession>, registry: Arc<Registry>) {
    info!("Interactive mode started. Waiting for JSON requests on stdin...");

    let stdin = std::io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to read stdin: {}", e);
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: Request = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                write_response(&Response::Error {
                    message: format!("Invalid JSON request: {}", e),
                });
                continue;
            }
        };

        if matches!(req, Request::Close) {
            write_response(&Response::Closed);
            break;
        }

        let resp = handle_request(req, &session, &registry).await;
        write_response(&resp);
    }

    info!("Interactive session ended");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.into()),
        )
        .with_target(false)
        .init();

    match cli.command {
        Commands::Run { show_browser } => {
            let mut app_config = if let Some(config_path) = &cli.config {
                Config::load_from_path(config_path)?
            } else {
                Config::load()
            };

            if show_browser {
                app_config.browser.headless = false;
            }

            let profile = BrowserProfile::from_config(&app_config.browser);
            let session = BrowserSession::new(profile)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create browser session: {}", e))?;
            let session = Arc::new(session);

            let registry = Arc::new(Registry::new());
            registry.register_default_actions().await;

            run_interactive(session, registry).await;
        }

        Commands::ListActions => {
            let registry = Registry::new();
            registry.register_default_actions().await;
            let defs = registry.action_definitions().await;
            let schema = registry.get_action_schema().await;
            let output = json!({
                "actions": convert_action_defs(&defs),
                "schema": schema,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }

        Commands::Tester => {
            println!("Launching FairyAction Tester...");
            println!("Note: Run 'fa-tester' directly for the interactive TUI tester.");
        }

        Commands::Config { action } => match action {
            ConfigAction::Show => {
                let config = Config::load();
                let json = serde_json::to_string_pretty(&config)?;
                println!("{}", json);
            }
            ConfigAction::Init => {
                let config = Config::default();
                config.save_to_path("fairy-action.json")?;
                println!("Configuration saved to fairy-action.json");
            }
            ConfigAction::Set { key, value } => {
                let mut config = Config::load();
                config.set(&key, &value)?;
                config.save_to_path("fairy-action.json")?;
                println!("Set {} = {}", key, value);
            }
        },
    }

    Ok(())
}
