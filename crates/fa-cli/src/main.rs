use clap::{Parser, Subcommand};
use fa_agent::service::Agent;
use fa_agent::views::AgentConfig;
use fa_config::Config;

#[derive(Parser)]
#[command(name = "fairy-action")]
#[command(about = "FairyAction - AI-powered browser automation agent")]
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
        #[arg(help = "The task description for the agent")]
        task: Vec<String>,

        #[arg(short, long, help = "Maximum number of steps")]
        max_steps: Option<usize>,

        #[arg(long, help = "Enable vision (screenshots)")]
        vision: bool,

        #[arg(short, long, help = "Save execution trace to file")]
        trace: Option<String>,

        #[arg(long, help = "Run in non-headless mode (show browser)")]
        show_browser: bool,

        #[arg(long, help = "LLM provider override")]
        provider: Option<String>,

        #[arg(long, help = "LLM model override")]
        model: Option<String>,

        #[arg(long, help = "LLM API key override")]
        api_key: Option<String>,

        #[arg(long, help = "LLM base URL override")]
        base_url: Option<String>,
    },

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
        Commands::Run {
            task,
            max_steps,
            vision,
            trace,
            show_browser,
            provider,
            model,
            api_key,
            base_url,
        } => {
            let task_str = task.join(" ");
            if task_str.is_empty() {
                anyhow::bail!("Task description is required. Usage: fairy-action run \"your task\"");
            }

            let mut app_config = if let Some(config_path) = &cli.config {
                Config::load_from_path(config_path)?
            } else {
                Config::load()
            };

            if show_browser {
                app_config.browser.headless = false;
            }
            if let Some(p) = provider {
                app_config.llm.provider = p;
            }
            if let Some(m) = model {
                app_config.llm.model = m;
            }
            if let Some(k) = api_key {
                app_config.llm.api_key = Some(k);
            }
            if let Some(u) = base_url {
                app_config.llm.base_url = Some(u);
            }

            let agent_config = AgentConfig {
                max_steps: max_steps.unwrap_or(100),
                max_actions_per_step: 10,
                use_vision: vision,
                save_trace: trace.is_some(),
                trace_path: trace.unwrap_or_else(|| "trace.jsonl".to_string()),
            };

            let mut agent = Agent::new(&task_str, agent_config, &app_config).await?;

            let ctrl_c = tokio::signal::ctrl_c();
            tokio::select! {
                result = agent.run() => {
                    match result {
                        Ok(step_result) => {
                            if let Some(output) = &step_result.final_output {
                                println!("\n✓ Task completed: {}", output);
                            } else if let Some(error) = &step_result.error {
                                println!("\n✗ Task failed: {}", error);
                            }
                            println!("Steps: {}", agent.step_count());
                        }
                        Err(e) => {
                            eprintln!("Agent error: {}", e);
                        }
                    }
                }
                _ = ctrl_c => {
                    println!("\nInterrupted by user. Shutting down...");
                }
            }
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
