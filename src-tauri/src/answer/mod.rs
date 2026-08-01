//! 答案提供商模块：DeepSeek / OpenAI / 自定义 OpenAI-compatible 流式适配器。

pub mod compatible;
pub mod deepseek;
pub mod openai;
mod prompt;
mod provider;

pub use provider::{
    AnswerConfig, AnswerError, AnswerEvent, AnswerProvider, AnswerRequest, CancellationToken,
    ProviderKind,
};
