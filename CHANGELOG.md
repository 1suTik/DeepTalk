# 更新日志（Task 交付记录）

本文件记录每个 Task 的交付内容、验证结果、提交信息与构建环境说明。每完成一个 Task 交付后在此追加记录。

---

## Task 4：实现模型管理、Silero VAD 和本地 Whisper 转写

**日期：** 2026-08-01

### 交付内容

- `src-tauri/models/models.json`：官方模型清单（id、下载 URL、SHA-256、大小、语言范围、运行档位），内嵌于程序
- `src-tauri/src/asr/model_manager.rs`：清单校验（必需字段）、**本地模型导入**（计算并登记 SHA-256、按大小匹配清单、临时文件 + 原子重命名、失败即删）、本地注册表持久化、`download_with_resume` 断点续传（Range 请求 + 大小校验）
- `src-tauri/src/vad/segmenter.rs`：VAD 分段状态机（30ms 帧、300ms 前置缓存、180ms 语音起段、600ms 静音收段、25s 强制切分；段不含尾部静音），与分类器解耦
- `src-tauri/src/vad/silero.rs`：Silero VAD v5 ONNX 分类器（ort 运行，维护 h/c 状态）
- `src-tauri/src/asr/whisper_worker.rs`：Whisper 转写原语（Vulkan 加载失败自动降级 CPU、`transcribe`/`transcribe_text`、16kHz 单声道 i16 WAV 读取）
- `tests/fixtures/audio/`：SAPI TTS 生成的中文/英文问题音频与静音测试音频（16kHz 单声道）
- 依赖：新增 `sha2`、`ndarray`（ort 输入标量）

### 验证结果

| 检查项 | 结果 |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml` | PASS（37 通过 + 2 忽略） |
| asr::model_manager::tests | PASS：错误哈希拒绝、正确哈希通过、导入登记与解析、按大小匹配清单、原子替换失败清理临时文件、断点续传（本地 mock HTTP 服务器）、大小不匹配检测、注册表持久化 |
| vad::segmenter::tests | PASS：静音不产出、两段语音正确分开（含前置缓存、不含尾部静音）、短噪声不起段、超长语音 25s 切分 |
| asr::whisper_worker::tests | PASS：缺失模型报错、WAV 读取、pcm→f32 映射 |
| 模型依赖集成测试（zh/en/silence fixtures 转写） | 标记 `#[ignore]`：**需要用户先本地导入 Whisper 模型**；导入后运行 `cargo test -- --ignored asr::whisper_worker` 验证 |

### 模型来源说明（按用户要求调整）

- v0.1.0 **不做模型自动下载**（HuggingFace 的 whisper.cpp 模型仓库需要登录，GitHub Release 无模型资产）
- 用户在设置/导入界面选择本地模型文件 → `import_model` 计算并登记 SHA-256，经校验后使用
- 官方清单（models.json）保留下载元数据与结构，供后续版本启用自动下载

### 提交信息

- 分支：`feat/task-4-local-asr`
- commit：`85225ba feat: add local streaming speech recognition`
- 状态：已推送，待确认后合并 `main`

---

## Task 3：实现 WASAPI 系统音频和可选麦克风采集

**日期：** 2026-08-01

### 交付内容

- `src-tauri/src/audio/resample.rs`：交错多声道浮点 → 16kHz 单声道 i16（均值 + 钳位防溢出）、一次性线性插值重采样、流式 `Resampler`
- `src-tauri/src/audio/level.rs`：RMS / 峰值 / RMS 分贝计算
- `src-tauri/src/audio/wasapi.rs`：WASAPI 采集（移植自 onetruedutchie-windows，MIT）：默认 render endpoint loopback（系统音频）、默认 capture endpoint（麦克风）、格式检测（IEEE float / PCM / extensible）、音量计算、停止信号
- `src-tauri/src/audio/mod.rs`：`AudioSource`（System/Microphone）、`AudioFrame`（来源标记 + 16kHz 单声道 i16 + 采集时刻）、采集线程启动函数；系统与麦克风数据写入不同 channel，不做 sample-by-sample 混合
- `src-tauri/examples/loopback_probe.rs`：真实设备探针
- `src-tauri/Cargo.toml`：windows crate 增加 `Win32_System_Com_StructuredStorage`、`Win32_System_Variant` feature（`IMMDevice::Activate` 需要）

### 验证结果

| 检查项 | 结果 |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml audio::resample::tests` | 先 FAIL（8 项未实现）→ 实现后 PASS |
| `cargo test --manifest-path src-tauri/Cargo.toml audio::` | PASS（11/11） |
| `cargo build --example loopback_probe` | PASS |
| 真实设备探针（播放 1kHz 测试音） | `LOOPBACK_OK frames=200 rms=0.0123 peak=0.0513`（非零 RMS）、`MIC_OK frames=291`、`PROBE_PASS` |
| 麦克风不可用时 | 探针输出 `MIC_UNAVAILABLE` 且系统音频不受影响（代码路径已验证） |

### 提交信息

- 分支：`feat/task-3-audio-capture`
- commit：`3617634 feat: capture separate system and microphone audio`
- 状态：已推送，待确认后合并 `main` 并打 `v0.1.0-m1` 里程碑 tag

### 说明

- 探针运行期间需要播放测试音频（如 1kHz 正弦）验证非零 RMS
- 采集输出契约：`AudioFrame { source, samples_16khz_mono: Vec<i16>, captured_at_ms }`

---

## Task 2：建立领域类型、Tauri 命令和双窗口 UI 骨架

**日期：** 2026-08-01

### 交付内容

- `src/types/domain.ts`：前后端领域契约（`Speaker`、`PipelineState`、`SessionState`、`CaptureSource`、`TranscriptSegment`、`DetectedQuestion`、`AnswerDraft`）
- `src/lib/events.ts`：稳定事件契约（capture-state、audio-level、transcript-pending/final、question-detected、answer-started/delta/completed 及载荷类型）
- `src/lib/tauri.ts`：Tauri invoke/事件监听封装（非 Tauri 环境安全降级）
- `src/features/meeting/OverlayPage.tsx` + `OverlayPage.test.tsx`：置顶会议面板（标题栏持续显示 AI 与采集状态）
- `src/components/CaptureIndicator.tsx`：采集状态指示（系统/麦克风/双路/未采集）
- `src-tauri/src/state.rs`：会话状态机 `SessionState`（Idle→Starting→Capturing→Stopping→Idle，Failed 可回 Idle）+ 7 项单元测试
- `src-tauri/src/commands.rs`：`start_session` / `stop_session` / `session_state` 命令
- `src-tauri/src/lib.rs`：注入 `SessionManager` 并注册命令
- `src-tauri/tauri.conf.json`：新增 `overlay` 窗口（始终置顶、可缩放、最小宽度 360px、默认不透明度 1.0 ≥ 70%）
- `src-tauri/icons/`：tauri-build 所需的窗口图标（占位图标，Task 11 细化）

### 验证结果

| 检查项 | 结果 |
|---|---|
| `npm test -- --run src/features/meeting/OverlayPage.test.tsx` | PASS（先 red 后 green） |
| `cargo test --manifest-path src-tauri/Cargo.toml state::tests` | PASS（7/7） |
| `npm test -- --run`（全量前端） | PASS |
| `npx tsc --noEmit` | PASS |
| `npm run build` | PASS |
| `cargo test`（完整依赖树） | PASS |

### 提交信息

- 分支：`feat/task-2-ui-skeleton`
- commit：`668863b feat: add visible meeting overlay and session state`
- 状态：已推送，待确认后合并 `main`

### 构建环境说明（重要）

- 首次完整编译依赖树（tauri + whisper.cpp Vulkan + ONNX Runtime），共 566 个 crate
- **260 字符路径限制**：项目路径深嵌套导致 MSVC 无法写入中间文件（cl.exe 不支持长路径，即使开启系统 LongPathsEnabled）。解决方案：cargo 构建目录迁移至短路径 `G:\t`（用户环境变量 `CARGO_TARGET_DIR=G:\t`，setx 持久化）
- 编译所需环境变量：`VULKAN_SDK`（1.4.350.0）、`LIBCLANG_PATH=C:\Program Files\LLVM\bin`（bindgen 需要）、`CMAKE_GENERATOR=Ninja`（whisper.cpp 构建，避免 MSBuild 长路径问题）
- 必须在 VS 开发者环境（`vcvars64.bat`）下执行 cargo 命令
- tauri 2.11.5 已移除窗口 opacity API，窗口不透明度为默认 1.0，满足「不得低于 70%」

---

## Task 1：初始化工程、测试框架与第三方登记

**日期：** 2026-08-01

### 交付内容

- `.gitignore`：排除 API Key、`.env`、SQLite、录音/转写、模型文件、`target/`、`node_modules/`、安装包输出
- `package.json`：React 19.2.8、Vite 8.2.0、Vitest 4.1.10、TypeScript 7.0.2、@tauri-apps/api 2.11.1、@tauri-apps/cli 2.11.4
- `vite.config.ts`、`tsconfig.json`、`index.html`、`src/main.tsx`、`src/test/setup.ts`
- `src-tauri/`：Cargo.toml（13 个直接依赖，含 whisper-rs 0.16.0[vulkan]、ort 2.0.0-rc.13、keyring 4.1.6 等）、tauri.conf.json、capabilities/default.json、最小可编译 lib.rs/main.rs、Cargo.lock（566 包）
- `THIRD_PARTY_NOTICES.md`：26 项第三方登记（URL/许可证/固定版本/复用板块/修改说明），含参考实现 commit `f3dca22`
- `scripts/verify-third-party.ps1`：校验登记表字段完整性 + 交叉校验 package.json/Cargo.toml 全部直接依赖

### 验证结果

| 检查项 | 结果 |
|---|---|
| `npm ls --depth=0` | 无 missing 依赖 |
| `cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps` | PASS（exit 0） |
| `npm run build` | PASS（exit 0） |
| `npx tsc --noEmit` | PASS |
| `powershell -ExecutionPolicy Bypass -File scripts/verify-third-party.ps1` | 输出 `Third-party manifest OK`（26 项） |

### 提交信息

- 分支：`main`
- commit：`5dfbeec chore: initialize tauri meeting assistant`（Task 1 在分支策略确定前完成，保留在 main）

### 环境安装记录

- Node.js 24.18.1（官方安装；opencode 自带 npm 损坏，无法执行任何命令）
- Rust 1.97.1 MSVC（rustup）
- Vulkan SDK 1.4.350.0（whisper-rs vulkan feature 编译期需要头文件）
- GitHub CLI 2.97.0；远程仓库 https://github.com/1suTik/Interview-Assistant---Deepseek.git（私有，默认分支 main）

### Git 策略变更

- 2026-08-01 起：每个 Task 在独立功能分支 `feat/task-N-描述` 开发，测试通过后 push 分支，确认后合并 `main`（详见 PROJECT_PLAN.md 3.1）
