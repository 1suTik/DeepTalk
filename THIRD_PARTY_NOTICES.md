# THIRD_PARTY_NOTICES

本项目（DeepTalk v0.1.1）使用的开源项目、许可证与复用记录。

**审计日期：** 2026-08-03（v0.1.1 公开发布）

**登记规则：**

- 任何直接依赖（`package.json` 的 `dependencies`/`devDependencies`、`src-tauri/Cargo.toml` 的 `[dependencies]`/`[build-dependencies]`）在引入前必须先在本表登记。
- 引擎代码许可证与模型文件许可证分别审计（见下文「模型许可证」一节）。
- 实际使用版本以 `package-lock.json` 与 `src-tauri/Cargo.lock` 为准；本表列出引入时的固定版本。

## 项目登记表

| 项目 | 用途 | URL | 许可证 | 固定版本/Commit | 复用板块 | 修改说明 | 状态 |
|---|---|---|---|---|---|---|---|
| whisper.cpp | 本地中英 ASR 推理核心 | https://github.com/ggerganov/whisper.cpp | MIT | 经 whisper-rs-sys 0.15.0 内置固定 revision（见 Cargo.lock） | 模型格式、推理核心、Vulkan/CPU 后端 | 通过 whisper-rs 绑定在独立工作线程运行，不直接修改上游源码；Vulkan 后端编译需 Vulkan SDK；模型文件由用户本地导入（`src-tauri/src/asr/whisper_worker.rs`） | 已引入 |
| whisper-rs | Rust 调用 whisper.cpp | https://codeberg.org/tazz4843/whisper-rs | Unlicense | 0.15.0（Cargo.lock 锁定） | `WhisperContext`、推理参数和 segment 读取 | 启用 `vulkan` feature（编译期需要 Vulkan SDK 头文件） | 已引入 |
| silero-vad | 语音活动检测 | https://github.com/snakers4/silero-vad | MIT（模型文件 MIT） | 模型 v6（silero_vad.onnx，SHA-256 由导入校验登记） | 30ms 帧 VAD 起止点检测（`src-tauri/src/vad/silero.rs`，经 Rust ort 运行） | 模型文件不提交 Git，保存于 `%LOCALAPPDATA%\MeetingAIAssistant\models\`，由用户本地导入 | 已引入 |
| tauri-apps/tauri | 桌面应用外壳 | https://github.com/tauri-apps/tauri | MIT OR Apache-2.0 | tauri 2.11.5 / tauri-build 2.6.3 | 主窗口、置顶窗口、Rust command/event、打包 | 未修改；权限按 capability 白名单最小化 | 已引入 |
| microsoft/windows-rs | Windows 原生 API | https://github.com/microsoft/windows-rs | MIT OR Apache-2.0 | 0.62.x | WASAPI、默认音频端点、Windows 凭据 API、窗口属性 | 仅启用所需 feature（Win32_Media_Audio、System_Com、Security_Credentials、UI_WindowsAndMessaging 等） | 已引入 |
| tokio-rs/tokio | 异步运行时 | https://github.com/tokio-rs/tokio | MIT | 1.53.x | channel、取消、超时、后台任务 | 未修改；仅启用 rt-multi-thread/macros/sync/time/fs/signal/io-util/net | 已引入 |
| rust-lang/futures-rs（crate 名 `futures-util`） | 异步流工具 | https://github.com/rust-lang/futures-rs | MIT OR Apache-2.0 | 0.3.33 | reqwest `bytes_stream` 的 StreamExt（`answer/provider.rs` SSE 解析） | 未修改 | 已引入 |
| seanmonstar/reqwest | 模型 API 调用 | https://github.com/seanmonstar/reqwest | MIT OR Apache-2.0 | 0.13.x | HTTPS、流式响应、超时和代理 | 未修改；默认 rustls TLS；启用 json 与 stream（SSE 流式） | 已引入 |
| rusqlite/rusqlite | 本地数据库 | https://github.com/rusqlite/rusqlite | MIT | 0.40.x | SQLite 连接、事务和迁移 | 启用 bundled，编译内置 SQLite；不保存 API Key | 已引入 |
| open-source-cooperative/keyring-rs | 密钥存储 | https://github.com/open-source-cooperative/keyring-rs | MIT OR Apache-2.0 | 4.1.x | Windows Credential Manager 后端 | 启用 windows-native-keyring-store；数据库只保存 provider 名称和 key 引用 | 已引入 |
| pykeio/ort | ONNX Runtime 绑定 | https://github.com/pykeio/ort | MIT OR Apache-2.0 | 2.0.0-rc.13 | Silero VAD 的 ONNX 推理 | 启用 ndarray/download-binaries/copy-dylibs/tracing；ONNX Runtime 预编译二进制 | 已引入 |
| serde-rs/serde | 序列化 | https://github.com/serde-rs/serde | MIT OR Apache-2.0 | serde 1.0.x / serde_json 1.0.x | derive、JSON 编解码 | 未修改 | 已引入 |
| dtolnay/thiserror | 错误类型 | https://github.com/dtolnay/thiserror | MIT OR Apache-2.0 | 2.0.19 | derive 错误类型 | 未修改 | 已引入 |
| tokio-rs/tracing | 结构化日志 | https://github.com/tokio-rs/tracing | MIT | 0.1.44 | tracing subscriber、span 计时 | 未修改；日志不得输出 key、完整资料内容或完整答案请求体 | 已引入 |
| J-F-Liu/lopdf | PDF 解析 | https://github.com/J-F-Liu/lopdf | MIT | 0.44.x | PDF 文本对象读取（`profile/extractor.rs`） | 本地提取文字，不上传原文件；限制页数与解压大小 | 已引入 |
| zip-rs/zip2 + tafia/quick-xml | DOCX 解析 | https://github.com/zip-rs/zip2 | MIT | zip 8.6.x / quick-xml 0.41.x | 解压 DOCX、读取 word/document.xml（`profile/extractor.rs`） | 只读 word/document.xml；不访问文档内 URL 或外部资源；限制条目数与解压大小 | 已引入 |
| facebook/react | UI 库 | https://github.com/facebook/react | MIT | react 19.2.x / react-dom 19.2.x | 状态组件、hooks、事件 | 未修改 | 已引入 |
| lucide-icons/lucide | 图标库 | https://github.com/lucide-icons/lucide | ISC | lucide-react 1.28.x | 状态与操作图标 | 未修改 | 已引入 |
| microsoft/TypeScript | TypeScript 编译器 | https://github.com/microsoft/TypeScript | Apache-2.0 | 7.0.x | 类型检查 | 未修改，开发依赖 | 已引入 |
| vitejs/vite | 构建工具 | https://github.com/vitejs/vite | MIT | vite 8.2.x / @vitejs/plugin-react 6.0.x | dev server、生产构建 | 未修改，开发依赖 | 已引入 |
| vitest-dev/vitest | 测试框架 | https://github.com/vitest-dev/vitest | MIT | vitest 4.1.x / jsdom 30.0.x | 单元测试、DOM 环境 | 未修改，开发依赖 | 已引入 |
| testing-library | React 测试工具 | https://github.com/testing-library/react-testing-library | MIT | @testing-library/react 16.3.x / @testing-library/jest-dom 7.0.x | render、DOM 断言 | 未修改，开发依赖 | 已引入 |
| tauri-apps/tauri（npm 侧） | Tauri JS API 与 CLI | https://github.com/tauri-apps/tauri | MIT OR Apache-2.0 | @tauri-apps/api 2.11.x / @tauri-apps/cli 2.11.x | invoke、event 监听、构建命令 | 未修改 | 已引入 |
| RustCrypto/sha2 | SHA-256 校验 | https://github.com/RustCrypto/hashes | MIT OR Apache-2.0 | 0.10（见 Cargo.lock） | 模型文件哈希校验（`asr/model_manager.rs`） | 未修改 | 已引入 |
| rust-ndarray/ndarray | 多维数组 | https://github.com/rust-ndarray/ndarray | MIT OR Apache-2.0 | 0.17（见 Cargo.lock） | ort 输入标量构造（`vad/silero.rs`） | 未修改；与 ort 的 ndarray feature 同版本 | 已引入 |

> 参考实现：`onetruedutchie-windows`（MIT，commit f3dca22）——Task 3 移植其 WASAPI Loopback 思路至 `src-tauri/src/audio/wasapi.rs`（重新组织为 audio 模块并按本项目事件契约输出 `AudioFrame`，保留 MIT 署名；不使用其网络服务、明文密钥与隐藏浮窗逻辑）。

## 模型许可证

| 模型 | 来源 | 许可证 | 说明 |
|---|---|---|---|
| Whisper large-v3-turbo 量化模型（ggml） | whisper.cpp 官方仓库发布 | MIT | 用户本地导入，SHA-256 校验登记；不放入安装包、不提交 Git |
| Silero VAD v6（silero_vad.onnx） | snakers4/silero-vad 官方 Release | MIT | 用户本地导入，SHA-256 校验登记；不提交 Git |

## 使用说明

1. 每次新增/升级依赖：先在本表登记或更新版本。
2. 传递依赖的完整许可证审计可通过 `cargo deny check licenses` 执行。
3. 本表列出直接引入项；实际传递依赖版本以 `Cargo.lock` 与 `package-lock.json` 为准。
