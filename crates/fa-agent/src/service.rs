use anyhow::{Context, Result};
use fa_browser::profile::BrowserProfile;
use fa_browser::session::BrowserSession;
use fa_config::Config;
use fa_dom::service::DomService;
use fa_dom::views::SerializedDOMState;
use fa_llm::base::ChatModel;
use fa_llm::factory::ChatModelFactory;
use fa_llm::messages::Message;
use fa_tools::actions::ActionContext;
use fa_tools::registry::Registry;
use fa_tools::params::ActionResult;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::views::{
    ActionStepResult, AgentConfig, AgentOutput, LoopDetection, StepResult,
};

pub struct Agent {
    config: AgentConfig,
    registry: Arc<Registry>,
    llm: Box<dyn ChatModel>,
    session: Arc<BrowserSession>,
    task: String,
    system_prompt: String,
    step_count: usize,
    loop_detection: LoopDetection,
    history: Vec<Message>,
    memory: Vec<String>,
    previous_goal: Option<String>,
}

impl Agent {
    pub async fn new(
        task: impl Into<String>,
        config: AgentConfig,
        app_config: &Config,
    ) -> Result<Self> {
        let profile = BrowserProfile::from_config(&app_config.browser);
        let session = BrowserSession::new(profile)
            .await
            .context("Failed to create browser session")?;
        let session = Arc::new(session);

        let llm = ChatModelFactory::create(
            &app_config.llm.provider,
            &app_config.llm.model,
            app_config.llm.api_key.as_deref().unwrap_or(""),
            app_config.llm.base_url.as_deref(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to create LLM: {}", e))?;

        let registry = Arc::new(Registry::new());
        registry.register_default_actions().await;

        let system_prompt = Self::build_system_prompt(&registry).await;

        Ok(Self {
            config,
            registry,
            llm,
            session,
            task: task.into(),
            system_prompt,
            step_count: 0,
            loop_detection: LoopDetection::default(),
            history: Vec::new(),
            memory: Vec::new(),
            previous_goal: None,
        })
    }

    pub async fn new_with_session(
        task: impl Into<String>,
        config: AgentConfig,
        app_config: &Config,
        session: Arc<BrowserSession>,
    ) -> Result<Self> {
        let llm = ChatModelFactory::create(
            &app_config.llm.provider,
            &app_config.llm.model,
            app_config.llm.api_key.as_deref().unwrap_or(""),
            app_config.llm.base_url.as_deref(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to create LLM: {}", e))?;

        let registry = Arc::new(Registry::new());
        registry.register_default_actions().await;

        let system_prompt = Self::build_system_prompt(&registry).await;

        Ok(Self {
            config,
            registry,
            llm,
            session,
            task: task.into(),
            system_prompt,
            step_count: 0,
            loop_detection: LoopDetection::default(),
            history: Vec::new(),
            memory: Vec::new(),
            previous_goal: None,
        })
    }

    async fn build_system_prompt(registry: &Registry) -> String {
        let action_desc = registry.build_action_description().await;

        format!(
            r#"You are a helpful AI assistant that controls a web browser. You can navigate to websites, click elements, type text, extract information, and more.

## Task
The user will give you a task. Break it down into steps and use the available actions to complete it.

## Available Actions
{action_desc}

## Rules
1. Analyze the page content and identify interactive elements by their index number (e.g., [0], [1], etc.)
2. Choose the most appropriate action based on the current page state
3. You can perform multiple actions in a single step if they are independent
4. When you have completed the task, use the "done" action with a summary of what you accomplished
5. If an action fails, try an alternative approach
6. Always read the page content before interacting with elements
7. Use "extract" to read page text when you need more information
8. Use "screenshot" to visually inspect the page if text content is insufficient
9. For navigation, always use absolute URLs with protocol (https://)

## Response Format
Respond with a JSON object containing:
- "current_state": An object with:
  - "thinking" (string, optional): Your reasoning about what to do next
  - "evaluation_previous_goal" (string, optional): Evaluate if the previous goal was achieved
  - "memory" (string, optional): Important information to remember for later steps
  - "next_goal" (string, optional): What you plan to do in this step
- "action": An array of action objects. Each action is an object with one key being the action name and the value being its parameters.
  Example: [{{"navigate": {{"url": "https://example.com"}}}}, {{"click": {{"index": 5}}}}]

## Important
- Element indices are 0-based and come from the DOM state provided
- Always check if an element exists before interacting with it
- If stuck in a loop, try a different approach"#
        )
    }

    pub async fn step(&mut self) -> Result<StepResult> {
        self.step_count += 1;
        info!(step = self.step_count, "Executing agent step");

        let dom_state = self.perceive().await?;
        let url = self.session.get_url().await.unwrap_or_default();
        let title = self.session.get_title().await.unwrap_or_default();

        let state_hash = Self::compute_state_hash(&url, &dom_state.llm_representation);
        if self.loop_detection.check(&state_hash) {
            warn!(step = self.step_count, "Loop detected, forcing action change");
        }

        let user_content = self.build_user_message(&url, &title, &dom_state).await;
        self.history.push(Message::user_text(user_content));

        let llm_response = self.decide().await?;

        let step_result = self.execute_actions(&llm_response.action, &url, &title).await;

        if step_result.is_done {
            info!(step = self.step_count, "Task completed");
        }

        self.previous_goal = llm_response.current_state.next_goal.clone();
        if let Some(mem) = &llm_response.current_state.memory {
            if !mem.is_empty() {
                self.memory.push(mem.clone());
            }
        }

        if self.config.save_trace {
            self.save_trace(&llm_response, &step_result).await;
        }

        Ok(step_result)
    }

    async fn perceive(&self) -> Result<SerializedDOMState> {
        let dom_state = DomService::get_dom_state(&self.session)
            .await
            .context("Failed to get DOM state")?;

        debug!(
            elements = dom_state.selector_map.len(),
            "DOM state perceived"
        );

        Ok(dom_state)
    }

    async fn decide(&mut self) -> Result<AgentOutput> {
        let messages = self.build_messages();

        debug!(msg_count = messages.len(), "Sending messages to LLM");

        let response = self.llm
            .invoke(messages, None)
            .await
            .map_err(|e| anyhow::anyhow!("LLM invocation failed: {}", e))?;

        let content = response.content.trim().to_string();

        let agent_output = self.parse_llm_response(&content)?;

        debug!(
            thinking = ?agent_output.current_state.thinking,
            actions = agent_output.action.len(),
            "LLM decision received"
        );

        self.history.push(Message::assistant(content));

        Ok(agent_output)
    }

    async fn execute_actions(
        &mut self,
        actions: &[Value],
        url: &str,
        title: &str,
    ) -> StepResult {
        let mut step_result = StepResult::default();

        let mut actions_to_execute = actions.to_vec();
        let max = self.config.max_actions_per_step.min(actions_to_execute.len());
        actions_to_execute.truncate(max);

        for action_value in &actions_to_execute {
            let (action_name, params) = self.extract_action(action_value);

            let ctx = ActionContext::new(
                self.session.clone(),
                url.to_string(),
                title.to_string(),
            );

            let result = match self.registry.execute(&action_name, params.clone(), ctx).await {
                Ok(r) => r,
                Err(e) => ActionResult::error(e.to_string()),
            };

            let step_action = ActionStepResult {
                action_name: action_name.clone(),
                params: params.clone(),
                success: result.success,
                output: result.output.clone(),
                error: result.error.clone(),
                extracted_content: result.extracted_content.clone(),
                include_in_memory: result.include_in_memory,
            };

            if result.include_in_memory {
                if let Some(content) = &result.output {
                    self.memory.push(content.clone());
                }
                if let Some(content) = &result.extracted_content {
                    self.memory.push(content.clone());
                }
            }

            if result.is_done {
                step_result.is_done = true;
                step_result.final_output = result.output;
                break;
            }

            let action_def = self.registry.get_action(&action_name).await;
            if let Some(def) = action_def {
                if def.terminates_sequence {
                    break;
                }
            }

            step_result.action_results.push(step_action);
        }

        step_result
    }

    fn extract_action(&self, action_value: &Value) -> (String, Value) {
        if let Some(obj) = action_value.as_object() {
            for (key, value) in obj {
                return (key.clone(), value.clone());
            }
        }
        ("unknown".to_string(), Value::Null)
    }

    fn parse_llm_response(&self, content: &str) -> Result<AgentOutput> {
        let cleaned = content.trim();

        let json_str = if cleaned.starts_with("```") {
            let lines: Vec<&str> = cleaned.lines().collect();
            let start = if lines[0].starts_with("```json") || lines[0].starts_with("```") {
                1
            } else {
                0
            };
            let end = lines.len().saturating_sub(if lines.last().map_or(false, |l| l.trim() == "```") { 1 } else { 0 });
            lines[start..end].join("\n")
        } else {
            cleaned.to_string()
        };

        let output: AgentOutput = serde_json::from_str(&json_str)
            .context("Failed to parse LLM response as AgentOutput")?;

        Ok(output)
    }

    async fn build_user_message(&self, url: &str, title: &str, dom_state: &SerializedDOMState) -> String {
        let mut parts = Vec::new();

        parts.push(format!("## Current Page\n- URL: {}\n- Title: {}\n", url, title));

        if self.config.use_vision {
            match self.session.screenshot().await {
                Ok(_) => {
                    parts.push("## Screenshot\n[Screenshot captured - page visual available]\n".to_string());
                }
                Err(e) => {
                    warn!(error = %e, "Failed to capture screenshot");
                }
            }
        }

        parts.push(format!(
            "## Interactive Elements\n{}\n",
            dom_state.llm_representation
        ));

        if let Some(ref goal) = self.previous_goal {
            parts.push(format!("## Previous Goal\n{}\n", goal));
        }

        if !self.memory.is_empty() {
            let recent_memory: Vec<_> = self.memory.iter().rev().take(10).collect();
            parts.push("## Memory\n".to_string());
            for mem in recent_memory.iter().rev() {
                parts.push(format!("- {}\n", mem));
            }
            parts.push("\n".to_string());
        }

        parts.join("\n")
    }

    fn build_messages(&self) -> Vec<Message> {
        let mut messages = vec![Message::system(&self.system_prompt)];
        messages.push(Message::user_text(
            format!("Your task: {}", self.task)
        ));
        messages.extend(self.history.clone());
        messages
    }

    fn compute_state_hash(url: &str, dom_repr: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let truncated = if dom_repr.len() > 2000 {
            &dom_repr[..2000]
        } else {
            dom_repr
        };
        truncated.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    async fn save_trace(&self, output: &AgentOutput, result: &StepResult) {
        if let Ok(trace) = serde_json::to_string(&json!({
            "step": self.step_count,
            "thinking": output.current_state.thinking,
            "next_goal": output.current_state.next_goal,
            "actions": output.action,
            "results": result.action_results,
            "is_done": result.is_done,
        })) {
            use tokio::io::AsyncWriteExt;
            match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.config.trace_path)
                .await
            {
                Ok(mut file) => {
                    if let Err(e) = file.write_all(format!("{}\n", trace).as_bytes()).await {
                        warn!(error = %e, "Failed to write trace");
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to open trace file");
                }
            }
        }
    }

    pub async fn run(&mut self) -> Result<StepResult> {
        info!(task = %self.task, "Starting agent run");

        let mut final_result = StepResult::default();

        for _ in 0..self.config.max_steps {
            match self.step().await {
                Ok(step_result) => {
                    if step_result.is_done {
                        final_result = step_result;
                        break;
                    }
                    final_result.action_results.extend(step_result.action_results);
                    if let Some(err) = step_result.error {
                        final_result.error = Some(err);
                        break;
                    }
                }
                Err(e) => {
                    error!(step = self.step_count, error = %e, "Agent step failed");
                    final_result.error = Some(format!("Step {} failed: {}", self.step_count, e));
                    break;
                }
            }
        }

        if !final_result.is_done && final_result.error.is_none() {
            warn!(
                steps = self.step_count,
                max = self.config.max_steps,
                "Agent reached max steps without completing task"
            );
            final_result.error = Some(format!(
                "Agent stopped after {} steps without completing the task",
                self.step_count
            ));
        }

        info!(
            steps = self.step_count,
            is_done = final_result.is_done,
            "Agent run completed"
        );

        Ok(final_result)
    }

    pub fn step_count(&self) -> usize {
        self.step_count
    }

    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    pub fn session(&self) -> &Arc<BrowserSession> {
        &self.session
    }
}
