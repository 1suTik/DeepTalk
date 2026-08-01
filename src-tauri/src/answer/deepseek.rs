//! DeepSeek 提供商：OpenAI-compatible chat streaming（默认 `https://api.deepseek.com/v1`）。

use tokio::sync::mpsc;

use super::provider::{ApiStyle, OpenAiCompatibleClient};
use super::{
    AnswerConfig, AnswerError, AnswerEvent, AnswerProvider, AnswerRequest, CancellationToken,
    ProviderKind,
};

pub const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/v1";

pub struct DeepSeekProvider {
    client: OpenAiCompatibleClient,
}

impl DeepSeekProvider {
    pub fn new(model: impl Into<String>, api_key: impl Into<String>) -> Result<Self, AnswerError> {
        let cfg = AnswerConfig::new(ProviderKind::DeepSeek, DEEPSEEK_BASE_URL, model, api_key);
        Ok(Self {
            client: OpenAiCompatibleClient::new(cfg, ApiStyle::ChatCompletions)?,
        })
    }
}

impl AnswerProvider for DeepSeekProvider {
    async fn stream_answer(
        &self,
        request: AnswerRequest,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<AnswerEvent>, AnswerError> {
        self.client.stream_answer(request, cancel).await
    }
}
