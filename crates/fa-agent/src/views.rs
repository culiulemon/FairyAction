use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    pub current_state: CurrentState,
    pub action: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentState {
    pub thinking: Option<String>,
    pub evaluation_previous_goal: Option<String>,
    pub memory: Option<String>,
    pub next_goal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StepResult {
    pub action_results: Vec<ActionStepResult>,
    pub is_done: bool,
    pub final_output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStepResult {
    pub action_name: String,
    pub params: serde_json::Value,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub extracted_content: Option<String>,
    pub include_in_memory: bool,
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub max_steps: usize,
    pub max_actions_per_step: usize,
    pub use_vision: bool,
    pub save_trace: bool,
    pub trace_path: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 100,
            max_actions_per_step: 10,
            use_vision: true,
            save_trace: false,
            trace_path: "trace.jsonl".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopDetection {
    pub recent_states: Vec<String>,
    pub max_history: usize,
    pub consecutive_repeats: usize,
    pub max_consecutive: usize,
}

impl Default for LoopDetection {
    fn default() -> Self {
        Self {
            recent_states: Vec::new(),
            max_history: 20,
            consecutive_repeats: 0,
            max_consecutive: 3,
        }
    }
}

impl LoopDetection {
    pub fn check(&mut self, state_hash: &str) -> bool {
        if let Some(last) = self.recent_states.last() {
            if last == state_hash {
                self.consecutive_repeats += 1;
                if self.consecutive_repeats >= self.max_consecutive {
                    return true;
                }
            } else {
                self.consecutive_repeats = 0;
            }
        }
        self.recent_states.push(state_hash.to_string());
        if self.recent_states.len() > self.max_history {
            self.recent_states.remove(0);
        }
        false
    }

    pub fn reset(&mut self) {
        self.recent_states.clear();
        self.consecutive_repeats = 0;
    }
}
