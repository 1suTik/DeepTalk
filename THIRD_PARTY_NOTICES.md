# THIRD_PARTY_NOTICES

本项目（Meeting AI Assistant v0.1.0）使用的开源项目、许可证与复用记录。

**审计日期：** 2026-08-01（Task 1 初始化）

**登记规则：**

- 任何直接依赖（`package.json` 的 `dependencies`/`devDependencies`、`src-tauri/Cargo.toml` 的 `[dependencies]`/`[build-dependencies]`）在引入前必须先在本表登记；`scripts/verify-third-party.ps1` 会交叉校验依赖树与本表。
- 引擎代码许可证与模型文件许可证分别审计（见下文“模型许可证”一节）。
- 实际使用版本以 `package-lock.json` 与 `src-tauri/Cargo.lock` 为准；本表列出引入时的固定版本。
- 未引入的项目（状态为“待引入”）在引入后必须更新为实际版本与复用记录。

## 项目登记表

| 项目 | 用途 | URL | 许可证 | 固定版本/Commit | 复用板块 | 修改说明 | 依赖匹配 | 状态 |
|---|---|---|---|---|---|---|---|---|
| onetruedutchie-windows | Windows 会议程序参考实现 | https://github.com/pawan0305/onetruedutchie-windows | MIT | f3dca22f8ab30a4f0234c9bba51a71b794ec5ea7（2026-05-29） | WASAPI Loopback（`audio.rs`）：默认 render endpoint loopback、默认 capture endpoint、格式检测、音量计算、停止信号；`src/overlay/` 双窗口思路 | Task 3 已移植至 `src-tauri/src/audio/wasapi.rs`（重新组织为 audio 模块并按本项目事件契约输出 `AudioFrame`，保留 MIT 署名）；不使用 deepgram.rs、明文 keys.json 与 click-through 隐藏浮窗逻辑 | - | 参考实现（Task 3 已移植） |
| whisper.cpp | 本地中英 ASR 推理核心 | https://github.com/ggerganov/whisper.cpp | MIT | 经 whisper-rs-sys 0.15.0 内置固定 revision（见 Cargo.lock） | 模型格式、推理核心、Vulkan/CPU 后端 | 通过 whisper-rs 绑定在独立工作线程运行，不直接修改上游源码；Vulkan 后端编译需 Vulkan SDK；模型文件由用户本地导入（`src-tauri/src/asr/whisper_worker.rs`） | - | 已引入（Task 4 直接使用） |
| whisper-rs | Rust 调用 whisper.cpp | https://codeberg.org/tazz4843/whisper-rs | Unlicense | 0.16.0 | `WhisperContext`、推理参数和 segment 读取 | 启用 `vulkan` feature（编译期需要 Vulkan SDK 1.4.350.0 头文件）；Cargo.lock 锁定 | whisper-rs | 已引入 |
| silero-vad | 语音活动检测 | https://github.com/snakers4/silero-vad | MIT（模型文件 MIT） | 模型 v5（2024-06-20 发布，silero_vad.onnx，SHA-256 由导入校验登记） | 30ms 帧 VAD 起止点检测（`src-tauri/src/vad/silero.rs`，经 Rust ort 运行） | 已引入；模型文件不提交 Git，保存于 `%LOCALAPPDATA%\MeetingAIAssistant\models\`；v0.1.0 由用户本地导入 | - | 已引入（Task 4） |
| tauri-apps/tauri | 桌面应用外壳 | https://github.com/tauri-apps/tauri | MIT OR Apache-2.0 | tauri 2.11.5 / tauri-build 2.6.3 | 主窗口、置顶窗口、Rust command/event、打包 | 未修改；权限按 capability 白名单最小化 | tauri, tauri-build | 已引入 |
| tauri-apps/plugins-workspace | Tauri 官方插件 | https://github.com/tauri-apps/plugins-workspace | MIT OR Apache-2.0 | 未引入（Task 2 起按需引入） | dialog、fs、shell、window-state、global-shortcut | 仅启用实际需要的权限并维护 capability 白名单 | - | 待引入（Task 2） |
| microsoft/windows-rs | Windows 原生 API | https://github.com/microsoft/windows-rs | MIT OR Apache-2.0 | 0.62.2 | WASAPI、默认音频端点、Windows 凭据 API、窗口属性 | 仅启用所需 feature（Win32_Media_Audio、System_Com、Security_Credentials、UI_WindowsAndMessaging 等）；不使用进程注入 API | windows | 已引入 |
| tokio-rs/tokio | 异步运行时 | https://github.com/tokio-rs/tokio | MIT | 1.53.1 | channel、取消、超时、后台任务 | 未修改；仅启用 rt-multi-thread/macros/sync/time/fs/signal/io-util/net | tokio | 已引入 |
| seanmonstar/reqwest | 模型 API 调用 | https://github.com/seanmonstar/reqwest | MIT OR Apache-2.0 | 0.13.4 | HTTPS、流式响应、超时和代理 | 未修改；默认 rustls TLS；启用 json 与 stream（SSE 流式） | reqwest | 已引入 |
| rusqlite/rusqlite | 本地数据库 | https://github.com/rusqlite/rusqlite | MIT | 0.40.1 | SQLite 连接、事务和迁移 | 启用 bundled，编译内置 SQLite；不保存 API Key | rusqlite | 已引入 |
| open-source-cooperative/keyring-rs | 密钥存储 | https://github.com/open-source-cooperative/keyring-rs | MIT OR Apache-2.0 | 4.1.6 | Windows Credential Manager 后端 | 启用 windows-native-keyring-store；数据库只保存 provider 名称和 key 引用 | keyring | 已引入 |
| pykeio/ort | ONNX Runtime 绑定 | https://github.com/pykeio/ort | MIT OR Apache-2.0 | 2.0.0-rc.13 | Silero VAD 的 ONNX 推理 | 启用 ndarray/download-binaries/copy-dylibs/tracing；ONNX Runtime 1.28 预编译二进制 | ort | 已引入 |
| serde-rs/serde | 序列化 | https://github.com/serde-rs/serde | MIT OR Apache-2.0 | serde 1.0.229 / serde_json 1.0.151 | derive、JSON 编解码 | 未修改 | serde, serde_json | 已引入 |
| dtolnay/thiserror | 错误类型 | https://github.com/dtolnay/thiserror | MIT OR Apache-2.0 | 2.0.19 | derive 错误类型 | 未修改 | thiserror | 已引入 |
| tokio-rs/tracing | 结构化日志 | https://github.com/tokio-rs/tracing | MIT | 0.1.44 | tracing subscriber、span 计时 | 未修改；日志不得输出 key、完整资料内容或完整答案请求体 | tracing | 已引入 |
| J-F-Liu/lopdf | PDF 解析 | https://github.com/J-F-Liu/lopdf | MIT | 未引入（Task 6） | PDF 文本对象读取 | 本地提取文字，不上传原文件 | - | 待引入（Task 6） |
| zip-rs/zip2 + tafia/quick-xml | DOCX 解析 | https://github.com/zip-rs/zip2 | MIT | 未引入（Task 6） | 解压 DOCX、读取 word/document.xml | 本地提取段落和标题，不访问文档内 URL 或外部资源 | - | 待引入（Task 6） |
| facebook/react | UI 库 | https://github.com/facebook/react | MIT | react 19.2.8 / react-dom 19.2.8 | 状态组件、hooks、事件 | 未修改 | react, react-dom | 已引入 |
| shadcn-ui/ui | UI 组件 | https://github.com/shadcn-ui/ui | MIT | 未引入（Task 2） | 状态组件、表单、按钮、无障碍交互 | 按项目主题重写布局，不复制第三方品牌视觉 | - | 待引入（Task 2） |
| lucide-icons/lucide | 图标库 | https://github.com/lucide-icons/lucide | ISC | 未引入（Task 2） | 状态与操作图标 | 未修改 | - | 待引入（Task 2） |
| microsoft/TypeScript | TypeScript 编译器 | https://github.com/microsoft/TypeScript | Apache-2.0 | 7.0.2 | 类型检查 | 未修改，开发依赖 | typescript | 已引入 |
| DefinitelyTyped | React 类型声明 | https://github.com/DefinitelyTyped/DefinitelyTyped | MIT | @types/react / @types/react-dom（跟随 npm 解析版本，见 package-lock.json） | React/ReactDOM 类型定义 | 未修改，开发依赖 | @types/react, @types/react-dom | 已引入 |
| vitejs/vite | 构建工具 | https://github.com/vitejs/vite | MIT | vite 8.2.0 / @vitejs/plugin-react 6.0.5 | dev server、生产构建 | 未修改，开发依赖 | vite, @vitejs/plugin-react | 已引入 |
| vitest-dev/vitest | 测试框架 | https://github.com/vitest-dev/vitest | MIT | vitest 4.1.10 / jsdom 30.0.1 | 单元测试、DOM 环境 | 未修改，开发依赖 | vitest, jsdom | 已引入 |
| testing-library | React 测试工具 | https://github.com/testing-library/react-testing-library | MIT | @testing-library/react 16.3.2 / @testing-library/jest-dom 7.0.0 | render、DOM 断言 | 未修改，开发依赖 | @testing-library/react, @testing-library/jest-dom | 已引入 |
| tauri-apps/tauri（npm 侧） | Tauri JS API 与 CLI | https://github.com/tauri-apps/tauri | MIT OR Apache-2.0 | @tauri-apps/api 2.11.1 / @tauri-apps/cli 2.11.4 | invoke、event 监听、构建命令 | 未修改；Rust 侧见上方 tauri 行 | @tauri-apps/api, @tauri-apps/cli | 已引入 |

## 模型许可证

| 模型 | 来源 | 许可证 | 说明 |
|---|---|---|---|
| Whisper large-v3-turbo 量化模型（ggml） | whisper.cpp 官方仓库发布 | MIT（ggml 模型仓库与权重） | 首次启动下载，SHA-256 校验；不放入安装包、不提交 Git |
| Silero VAD v5（silero_vad.onnx） | snakers4/silero-vad 官方 Release | MIT | 首次启动下载，SHA-256 校验；不提交 Git |

## 使用说明

1. 每次新增/升级依赖：先在本表登记或更新版本，再执行 `scripts/verify-third-party.ps1`。
2. 传递依赖的完整许可证审计在 Task 11 通过 `cargo deny check licenses` 执行。
3. 被标记为“待引入”的项目在引入后须将“固定版本/Commit”与“状态”更新为实际值。
