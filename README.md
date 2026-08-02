# Meeting AI Assistant

Windows 11 实时会议 AI 辅助程序：本地完成中英语音转写，识别提问，并通过 DeepSeek / OpenAI / 自定义 OpenAI 兼容接口流式显示建议答案。

## 功能

- **本地语音转写**：WASAPI 采集系统音频（loopback）与可选麦克风，Silero VAD 分段，Whisper（Vulkan，失败自动回退 CPU）本地转写；中英双语自动识别。
- **问题检测**：识别完整问题后自动触发生成答案；20 秒窗口 + 30 秒去重 + 分片续接抑制；低置信度问题提供手动生成。
- **流式答案**：DeepSeek / OpenAI / 自定义 OpenAI 兼容服务；SSE 流式输出，固定「短答 → 要点 → 追问」三段结构；取消、重新生成、复制、固定记录（固定后新问题排队）。
- **对话式界面**：主界面对话流（面试官问题 + 助手答案气泡）；置顶小窗口同步显示最新问题与流式短答。
- **本地资料**：导入 PDF / DOCX / TXT / Markdown 简历与岗位说明，答案结合命中的资料片段；资料只在本机解析与检索。
- **隐私与保留**：音频不出本机；API Key 存入 Windows Credential Manager；会议历史本地 SQLite 存储，默认 7 天自动清理。

## 模型（本地导入，不做自动下载）

| 模型 | 用途 | 体积 | 来源 |
|---|---|---|---|
| Whisper large-v3-turbo (Q5_0) | 中英转写 | ≈ 574 MB | whisper.cpp 官方 Release（HuggingFace） |
| Whisper base (Q5_0) | CPU 回退 | ≈ 61 MB | 同上 |
| Silero VAD v6 (16k ONNX) | 语音活动检测 | ≈ 2.3 MB | snakers4/silero-vad 官方仓库 |

将下载好的模型文件放入 `%LOCALAPPDATA%\MeetingAIAssistant\models\`，在设置页点击「扫描并校验」；SHA-256 校验通过后自动登记。校验失败（文件损坏）会被拒绝并提示重新选择。

## 快速开始

```powershell
npm install
npm run tauri dev
```

首次使用：
1. 设置页配置答案服务（DeepSeek / OpenAI / Custom）与 API Key，点击「连接测试」。
2. 导入语音模型（见上节）。
3. 主界面点击「开始会话」，播放会议音频即可。

## 常用命令

```powershell
npm test -- --run                # 前端测试
npm run build                     # 前端构建
cargo test --manifest-path src-tauri/Cargo.toml          # Rust 测试（模型依赖用例为 #[ignore]）
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored asr::whisper_worker   # 真实模型转写用例
npm run tauri build               # 生成 Windows NSIS 安装包
powershell -ExecutionPolicy Bypass -File scripts/verify-third-party.ps1
```

## 文档

- `docs/architecture.md`：模块架构与数据流
- `docs/build.md`：可复现构建环境
- `docs/troubleshooting.md`：常见问题排查

## 许可证

MIT。第三方依赖清单与许可证见 `THIRD_PARTY_NOTICES.md`。
