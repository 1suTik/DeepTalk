# 更新日志（Task 交付记录）

本文件记录每个 Task 的交付内容、验证结果、提交信息与构建环境说明。每完成一个 Task 交付后在此追加记录。

---

## Task 6：实现资料导入与本地相关片段匹配

**日期：** 2026-08-01

### 交付内容

- `src-tauri/src/profile/extractor.rs`：本地文档解析（PDF via lopdf / DOCX via zip+quick-xml / TXT / Markdown），安全限制：单文件 ≤ 5MB、PDF ≤ 500 页、DOCX 条目 ≤ 200 且解压 ≤ 20MB；**只读 word/document.xml，不访问文档内 URL 或外部资源**；规范化去除控制字符与重复空白
- `src-tauri/src/profile/importer.rs`：资料导入管理（最多 10 份、同路径去重、启用/移除、原子写本地 JSON 存储，存本机不上传原文件）
- `src-tauri/src/profile/matcher.rs`：确定性关键词匹配——400-800 字符切块（80 字符重叠，超长段落内部分段）、CJK 单字 + ASCII 词 token、标题加权（1.5×）+ BM25 风格评分；最多 4 个片段、总长 ≤ 6000 字符
- `src/features/profile/ProfileLibraryPage.tsx` + 测试：资料列表、导入（达上限禁用）、启用/停用、移除
- `tests/fixtures/documents/`：sample.md / sample.docx（含外部 hyperlink 关系，验证不访问）/ sample.pdf（lopdf 合法生成）
- 依赖：lopdf 0.44.0、zip 8.6.0、quick-xml 0.41.0

### 验证结果

| 检查项 | 结果 |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml profile::extractor::tests` | 先 FAIL（20 项未实现）→ 实现后 PASS |
| 解析安全测试 | PDF/DOCX/TXT/MD 正常解析、空文件拒绝、>5MB 拒绝、损坏 ZIP 拒绝、含外部关系 DOCX 不访问外部 URL 全部 PASS |
| `cargo test --manifest-path src-tauri/Cargo.toml profile::` | PASS：切块边界与重叠、中英文 token、相关文档优先命中、英文查询、结果数量与总长上限、标题加权、确定性、导入/去重/上限/启停 |
| `npm test -- --run src/features/profile` | PASS（4/4） |
| 全量 `cargo test` | PASS（70 通过 + 2 忽略）；`npm test`（5）`tsc`/`build` PASS |
| `scripts/verify-third-party.ps1` | `Third-party manifest OK`（28 项，含 Task 4 遗留的 sha2/ndarray 补登记） |

### 提交信息

- 分支：`feat/task-6-profile-library`
- commit：`1c1f2b0`（功能）
- 状态：已推送，待确认后合并 `main`

### 说明

- 匹配测试验证：「音频延迟优化」类问题优先命中包含 WASAPI/VAD/Whisper 的资料片段
- 资料匹配的会议侧选择（最多 3 份启用）与答案拼装将在 Task 9 会话编排中接入

---

## Task 5：实现问题检测、去重与上下文窗口

**日期：** 2026-08-01

### 交付内容

- `src-tauri/src/question/normalizer.rs`：文本规范化（全角→半角、去标点、折叠空白、小写），用于去重哈希与检索匹配
- `src-tauri/src/question/detector.rs`：轻量本地问题检测器
  - 规则覆盖：中文句末疑问词（吗/呢/吧）、中文疑问词（为什么/怎么/如何/什么/多少/是否/能否/哪些/哪里/谁）、中文命令式（请介绍/介绍一下/谈谈/说说/讲讲/解释/举例/说明/描述）、英文疑问词开头（who/what/why/how/does/is/can 等）、英文命令式（describe/explain/tell me/what about）
  - 置信度分级：**Auto ≥ 0.65**（自动生成）、**Maybe 0.40-0.64**（仅显示「可能的问题」，用户点击生成）、**< 0.40 忽略**
  - 最近 20 秒 remote 最终文本窗口，相邻短句自动合并；触发成功后窗口清空
  - 标准化文本哈希 30 秒内去重
  - `check_manual`：全局快捷键手动提交（不做去重），整窗作为问题

### 验证结果

| 检查项 | 结果 |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml question::` | 先 FAIL（12 项未实现）→ 实现后 PASS |
| 语料表测试 | 中文疑问句/英文疑问句/命令式面试题/普通陈述（中英）/过短文本/被截断问题/置信度分带/相邻短句合并/30s 去重/窗口清理/手动触发 全部 PASS |
| 全量 `cargo test` | PASS（50 通过 + 2 忽略） |

### 提交信息

- 分支：`feat/task-5-question-detection`
- commit：`143fa13`（功能）
- 状态：已推送，待确认后合并 `main` 并打 `v0.1.0-m2` 里程碑 tag

### 说明

- 检测器只消费 remote 源最终文本（`push_final`），与本地（麦克风）来源无关
- 全局快捷键的手动提交能力将在 Task 9 的会话编排中接入

---

## Task 4：实现模型管理、Silero VAD 和本地 Whisper 转写

**日期：** 2026-08-01

### 交付内容

- `src-tauri/models/models.json`：官方模型清单（id、下载 URL、SHA-256、大小、语言范围、运行档位），编译期内嵌
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

### 模型验证（2026-08-01，用户导入 ggml-large-v3-turbo-q5_0.bin）

- 实测转写（Vulkan 后端，RTX 3060 Ti）：zh `请介绍一下你负责的项目`、en `What was the hardest problem you solved?`、silence `''`（空）
- 已用真实 SHA-256 `394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2` 回填 models.json 的 large-v3-turbo 条目
- **修复 1 — 多显卡 Vulkan 崩溃**：机器含虚拟显示适配器，ggml-vulkan 枚举全部设备导致访问违规；worker 默认设 `GGML_VK_VISIBLE_DEVICES=0`（用户显式设置时不覆盖）
- **修复 2 — 并行 GPU 上下文崩溃**：ggml-vulkan 不允许同进程并行多个 GPU 上下文；WhisperWorker 增加全局互斥锁串行化
- **修复 3 — 静音幻觉**：whisper 对纯静音输出「Thank you.」；增加 RMS 静音门控（<0.005 直接跳过）+ no_speech_probability ≥0.6 段过滤
- 模型依赖测试（`#[ignore]`）在导入模型后全部 PASS：`cargo test -- --ignored asr::whisper_worker`

### 提交信息

- 分支：`feat/task-4-local-asr`
- commit：`85225ba`（功能）、`367e1f0`（CHANGELOG）、`1b34de7`（Vulkan 设备选择与静音门控修复）、`81a6930`（模型验证记录）、`5a549c7`（models.json 清单入库修复）
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
- commit：`3617634`（功能）、`d69dc70`（CHANGELOG 定稿）、`c95d387`（audio 模块导出与构建脚本）、`9aafb34`（警告清理）
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
- commit：`668863b`（UI 骨架与会话状态）、`7bdef21`（CHANGELOG 创建）、`8209cec`（overlay 预览接线）
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

---

## 文档维护说明

- 本文件必须保持 UTF-8（无 BOM）编码；禁止使用 PowerShell `Set-Content` 回写（PS 5.1 默认按 GBK 解码导致乱码）
- 每次 Task 交付：追加条目（交付内容 / 验证结果 / 提交信息 / 环境说明），随功能分支一起提交
