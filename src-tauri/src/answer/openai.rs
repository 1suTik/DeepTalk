//! OpenAI 提供商：官方 Responses API 流式事件（默认 `https://api.openai.com/v1`）。

use tokio::sync::mpsc;

use super::provider::{ApiStyle, OpenAiCompatibleClient};
use super::{
    AnswerConfig, AnswerError, AnswerEvent, AnswerProvider, AnswerRequest, CancellationToken,
    ProviderKind,
};

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAiProvider {
    client: OpenAiCompatibleClient,
}

impl OpenAiProvider {
    pub fn new(model: impl Into<String>, api_key: impl Into<String>) -> Result<Self, AnswerError> {
        let cfg = AnswerConfig::new(ProviderKind::OpenAi, OPENAI_BASE_URL, model, api_key);
        Ok(Self {
            client: OpenAiCompatibleClient::new(cfg, ApiStyle::Responses)?,
        })
    }
}

impl AnswerProvider for OpenAiProvider {
    fn stream_answer<'a>(
        &'a self,
        request: AnswerRequest,
        cancel: CancellationToken,
    ) -> futures_util::future::BoxFuture<'a, Result<mpsc::Receiver<AnswerEvent>, AnswerError>> {
        Box::pin(async move { self.client.stream_answer(request, cancel).await })
    }
}
