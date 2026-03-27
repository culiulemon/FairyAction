use crate::base::ChatModel;
use crate::base::LlmError;
use crate::openai::ChatOpenAI;

pub struct ChatModelFactory;

impl ChatModelFactory {
    pub fn create(
        provider: &str,
        model: &str,
        api_key: &str,
        base_url: Option<&str>,
    ) -> Result<Box<dyn ChatModel>, LlmError> {
        match provider {
            "openai" => {
                let mut llm = ChatOpenAI::new(api_key, model);
                if let Some(url) = base_url {
                    llm = llm.with_base_url(url);
                }
                Ok(Box::new(llm))
            }
            _ => Err(LlmError::ModelNotFound(format!(
                "Unknown provider: {}",
                provider
            ))),
        }
    }
}
