# 会议助手项目简介

本项目是一个 Windows 实时会议 AI 辅助程序，在本地完成中英语音转写、问题检测，并通过云端 API 流式显示建议答案。

## 音频采集与延迟优化

系统音频通过 WASAPI loopback 采集，48kHz 双声道浮点输入，重采样为 16kHz 单声道 i16。音频延迟优化方案包括：200ms 采集缓冲区、每 800ms 滚动窗口生成临时转写结果、Silero VAD 分段减少无效转写，实测问题结束到答案首字 P50 不超过 3 秒。

## 本地转写

Whisper large-v3-turbo 量化模型在 RTX 3060 Ti 上通过 Vulkan 推理，Vulkan 初始化失败时自动降级 CPU 模型。Silero VAD 以 30ms 帧检测语音起止，180ms 语音起段、600ms 静音收段、单段最长 25 秒。

## 数据隐私

音频绝不发送到答案 API，只发送最终文本与命中的资料片段。会议记录仅保存在本机，默认 7 天后自动清除。

## 技术栈

Tauri 2、Rust、React、TypeScript、WASAPI、whisper.cpp、Silero VAD、SQLite、DeepSeek/OpenAI 流式 API。
