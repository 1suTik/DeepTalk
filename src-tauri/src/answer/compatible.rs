//! 自定义 OpenAI-compatible 提供商（默认指向本地 `http://127.0.0.1:11434/v1`）。
//!
//! 只连接用户已启动的 Ollama / LM Studio 等本地 OpenAI-compatible 服务，
//! 不检测、安装或管理 Ollama 与本地答案模型。

use tokio::sync::mpsc;

use super::provider::{ApiStyle, OpenAiCompatibleClient};
use super::{
    AnswerConfig, AnswerError, AnswerEvent, AnswerProvider, AnswerRequest, CancellationToken,
    ProviderKind,
};

pub const CUSTOM_DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434/v1";

pub struct CompatibleProvider {
    client: OpenAiCompatibleClient,
}

impl CompatibleProvider {
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, AnswerError> {
        let cfg = AnswerConfig::new(ProviderKind::Custom, base_url, model, api_key);
        Ok(Self {
            client: OpenAiCompatibleClient::new(cfg, ApiStyle::ChatCompletions)?,
        })
    }

    /// 使用默认本地地址 `http://127.0.0.1:11434/v1`（Ollama 预设）。
    pub fn local_default(model: impl Into<String>) -> Result<Self, AnswerError> {
        Self::new(CUSTOM_DEFAULT_BASE_URL, model, "")
    }
}

impl AnswerProvider for CompatibleProvider {
    async fn stream_answer(
        &self,
        request: AnswerRequest,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<AnswerEvent>, AnswerError> {
        self.client.stream_answer(request, cancel).await
    }
}
