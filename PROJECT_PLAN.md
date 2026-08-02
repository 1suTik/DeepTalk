# Windows 实时会议 AI 辅助程序 v0.1.0 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建一个 Windows 11 桌面程序，本地完成中英语音转写，识别提问，并通过 DeepSeek、OpenAI 或自定义兼容接口流式显示建议答案。

**Version:** `v0.1.0`，个人使用的首个可交付原型。

**Architecture:** 使用 Tauri 2 承载 React 主窗口与始终置顶的会议面板；Rust 后端通过 WASAPI 分别采集系统音频和可选麦克风，通过程序内置的 Whisper 模型管理器完成本地转写。最终文本经过本地问题检测和资料匹配后发送给可替换的答案提供商，会议记录仅保存到本机并在 7 天后自动清除。v0.1.0 默认使用云端答案 API，不依赖 Ollama；自定义 OpenAI-compatible Provider 可以连接用户自行运行的 Ollama 或 LM Studio。

**Tech Stack:** Windows 11、Tauri 2、Rust stable MSVC、React、TypeScript、Vite、Vitest、Rust cargo test、WASAPI、whisper.cpp/whisper-rs、Silero VAD、SQLite、Windows Credential Manager、DeepSeek/OpenAI 流式 API。

---

## 1. 产品边界与验收目标

### 1.1 首版包含

- 接入腾讯会议、Teams、Zoom、浏览器会议等软件的默认播放设备音频，不修改或注入会议软件进程。
- 默认识别系统音频；麦克风转写由用户显式开启，两路音频和文字保持独立来源标记。
- 中英双语自动识别，优先支持普通话、英文以及常见技术词汇混说。
- 识别到完整问题后自动触发生成，并允许取消、重新生成和复制。
- 置顶小面板显示：AI 状态、采集状态、识别问题、20-40 秒可口述短答、展开要点和可能追问。
- 导入 PDF、DOCX、TXT、Markdown 格式的简历、项目经历和岗位说明；资料只在本机解析和检索。
- 支持 DeepSeek、OpenAI 和自定义 OpenAI 兼容服务；API Key 存入 Windows Credential Manager。
- 本地保存会议历史，默认 7 天自动删除；用户可固定单场记录或立即清除全部数据。
- 语音模型由**用户本地导入**（v0.1.0 约定，不做程序自动下载）：设置页提供导入/校验入口，`ModelManager::import_model` 计算并登记 SHA-256；校验失败拒绝导入。提供轻量 CPU 回退模型（Vulkan 初始化失败时自动回退，延迟可能增加）。
- Whisper 模型不放入安装包，默认保存到 `%LOCALAPPDATA%\MeetingAIAssistant\models\`，不得提交到 Git。
- 答案生成默认使用 DeepSeek/OpenAI API；高级用户可将 Custom Provider 指向本机 OpenAI-compatible 服务。

### 1.2 明确不包含

- 静默后台采集。
- 不向腾讯会议自动输入文字，不控制麦克风自动发言，不提供 TTS 代答。
- 不做账号、支付、云同步、团队管理、远程控制或 macOS 版本。
- 不在 v0.1.0 中安装、启动、更新或管理 Ollama/LM Studio，也不负责下载本地答案大模型。
- 不承诺在第三方网络或模型服务异常时仍达到 1-3 秒答案首字延迟。

### 1.3 可测量验收指标

| 指标 | 首版目标 |
|---|---|
| 音频启动 | 点击开始后 1 秒内音量表出现有效数据 |
| 本地转写 | 说话过程中 1.5 秒内出现临时文本 |
| 问题结束到答案首字 | 正常网络下 P50 不超过 3 秒，P95 不超过 5 秒 |
| 中文识别 | 安静环境普通话测试集字错率不高于 15% |
| 中英混说 | 自建技术面试测试集中关键词召回率不低于 85% |
| 稳定性 | 连续运行 60 分钟无崩溃，断网恢复后可继续生成 |
| 隐私 | 音频不发送到答案 API；只发送最终文本和命中的资料片段 |
| 数据清理 | 超过 7 天且未固定的记录在启动时和每日维护时删除 |

---

## 2. 开源项目与复用板块

实现前必须将实际使用版本、commit、许可证和修改说明写入 `THIRD_PARTY_NOTICES.md`。引擎代码许可证与模型文件许可证分别审计。

| 开源项目 | 计划用途 | 许可证基线 | 复用板块 | 集成方式 |
|---|---|---|---|---|
| [pawan0305/onetruedutchie-windows](https://github.com/pawan0305/onetruedutchie-windows) | Windows 会议程序参考实现 | MIT | `src-tauri/src/audio.rs` 的 WASAPI Loopback、`commands.rs` 的会话编排、`src/overlay/` 的双窗口思路、音量事件和重连模式 | 只移植经审计的必要代码并保留 MIT 署名；不直接整仓作为产品底座 |
| [ggerganov/whisper.cpp](https://github.com/ggerganov/whisper.cpp) | 本地中英 ASR | MIT | 模型格式、推理核心、流式识别示例、GPU/CPU 后端 | 通过 `whisper-rs` 绑定在独立工作线程运行；高质量模型使用 Vulkan/NVIDIA GPU，失败时降级 CPU |
| [tazz4843/whisper-rs](https://github.com/tazz4843/whisper-rs) | Rust 调用 whisper.cpp | MIT/Apache-2.0 | `WhisperContext`、推理参数和 segment 读取 | Cargo 依赖；锁定版本及底层 whisper.cpp revision |
| [snakers4/silero-vad](https://github.com/snakers4/silero-vad) | 语音活动检测 | MIT | Silero VAD ONNX 模型 | 通过 Rust `ort` 运行；30 ms 音频帧输入，用于起止点检测 |
| [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | 桌面应用外壳 | MIT/Apache-2.0 | 主窗口、置顶窗口、Rust command/event、打包 | Tauri 2 正式依赖 |
| [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | Tauri 官方插件 | MIT/Apache-2.0 | dialog、fs、shell、window-state、global-shortcut | 仅启用实际需要的权限并维护 capability 白名单 |
| [microsoft/windows-rs](https://github.com/microsoft/windows-rs) | Windows 原生 API | MIT/Apache-2.0 | WASAPI、默认音频端点、Windows 凭据 API、窗口属性 | Rust `windows` crate；不使用进程注入 API |
| [tokio-rs/tokio](https://github.com/tokio-rs/tokio) | 异步任务 | MIT | channel、取消、超时、后台任务 | Rust async runtime |
| [seanmonstar/reqwest](https://github.com/seanmonstar/reqwest) | 模型 API 调用 | MIT/Apache-2.0 | HTTPS、流式响应、超时和代理 | DeepSeek/OpenAI provider 的公共传输层 |
| [rusqlite/rusqlite](https://github.com/rusqlite/rusqlite) | 本地数据库 | MIT | SQLite 连接、事务和迁移 | 保存会议、转写、答案和资料元数据 |
| [open-source-cooperative/keyring-rs](https://github.com/open-source-cooperative/keyring-rs) | 密钥存储 | MIT/Apache-2.0 | Windows Credential Manager 后端 | 保存 API Key；数据库只保存 provider 名称和 key 引用 |
| [J-F-Liu/lopdf](https://github.com/J-F-Liu/lopdf) | PDF 解析 | MIT | PDF 文本对象读取 | 本地提取文字，不上传原文件 |
| [zip-rs/zip2](https://github.com/zip-rs/zip2) + [tafia/quick-xml](https://github.com/tafia/quick-xml) | DOCX 解析 | MIT | 解压 DOCX、读取 `word/document.xml` | 本地提取段落和标题 |
| [facebook/react](https://github.com/facebook/react) + [shadcn-ui/ui](https://github.com/shadcn-ui/ui) + [lucide-icons/lucide](https://github.com/lucide-icons/lucide) | UI | MIT/MIT/ISC | 状态组件、表单、按钮、图标和无障碍交互 | 按项目主题重写布局，不复制第三方品牌视觉 |

### 不复用的参考项目板块

- 不使用 OneTrueDutchie 的 `deepgram.rs`，因为本项目的语音识别默认在本地运行。
- 不使用其“翻译每个 chunk”的主流程；本项目只在检测到提问后调用答案模型。
- 不直接复用明文 `keys.json`；改用 Windows Credential Manager。
- 不复用 click-through 隐藏浮窗逻辑；会议面板保持可操作并持续显示 AI/采集状态。

---

## 3. 目标目录与模块职责

```text
meeting-ai-assistant/
├─ PROJECT_PLAN.md
├─ README.md
├─ THIRD_PARTY_NOTICES.md
├─ package.json
├─ vite.config.ts
├─ src/
│  ├─ app/App.tsx
│  ├─ app/router.tsx
│  ├─ components/AudioMeters.tsx
│  ├─ components/AnswerCard.tsx
│  ├─ components/CaptureIndicator.tsx
│  ├─ components/TranscriptFeed.tsx
│  ├─ features/dashboard/DashboardPage.tsx
│  ├─ features/meeting/MeetingPage.tsx
│  ├─ features/meeting/OverlayPage.tsx
│  ├─ features/meeting/useSessionEvents.ts
│  ├─ features/profile/ProfileLibraryPage.tsx
│  ├─ features/settings/SettingsPage.tsx
│  ├─ lib/events.ts
│  ├─ lib/tauri.ts
│  └─ types/domain.ts
├─ src-tauri/
│  ├─ Cargo.toml
│  ├─ tauri.conf.json
│  ├─ capabilities/default.json
│  ├─ migrations/001_initial.sql
│  ├─ benches/pipeline_latency.rs（Task 10）
│  └─ src/
│     ├─ lib.rs
│     ├─ commands.rs
│     ├─ state.rs
│     ├─ session.rs          （会话编排：事件契约、取消/排队竞争策略、TauriSink）
│     ├─ pipeline.rs         （生产流水线：WASAPI→VAD→Whisper 拼帧转写）
│     ├─ audio/{mod.rs,wasapi.rs,resample.rs,level.rs}
│     ├─ asr/{mod.rs,model_manager.rs,whisper_worker.rs}
│     ├─ vad/{mod.rs,silero.rs,segmenter.rs}
│     ├─ question/{mod.rs,detector.rs,normalizer.rs}
│     ├─ answer/{mod.rs,provider.rs,deepseek.rs,openai.rs,compatible.rs,prompt.rs}
│     ├─ profile/{mod.rs,importer.rs,extractor.rs,matcher.rs}
│     └─ storage/{mod.rs,database.rs,credentials.rs,retention.rs}
├─ tests/fixtures/audio/
├─ tests/fixtures/documents/
├─ tests/manual/（Task 10）
├─ docs/（Task 10-11）
└─ scripts/verify-third-party.ps1
```

### 核心事件契约

前后端只通过稳定事件传输领域数据，不直接暴露数据库或音频实现细节。

```typescript
export type Speaker = "remote" | "local";
export type PipelineState =
  | "idle"
  | "capturing"
  | "transcribing"
  | "generating"
  | "error";

export interface TranscriptSegment {
  id: string;
  speaker: Speaker;
  text: string;
  startedAtMs: number;
  endedAtMs: number;
  isFinal: boolean;
}

export interface DetectedQuestion {
  id: string;
  sourceSegmentIds: string[];
  normalizedText: string;
  confidence: number;
  detectedAtMs: number;
}

export interface AnswerDraft {
  questionId: string;
  shortAnswer: string;
  keyPoints: string[];
  followUps: string[];
  status: "streaming" | "complete" | "cancelled" | "failed";
}
```

### 3.1 Git 与 GitHub 策略

- GitHub 仓库默认使用私有仓库；课程提交时再按导师要求添加访问权限或导出源码。
- **分支策略：每个 Task 在独立的功能分支上开发，确认无误后合并回 `main`。** 每个 Task 开始时从最新 `main` 创建 `feat/task-N-<简短描述>` 分支（如 `feat/task-2-ui-skeleton`）；在分支上完成开发与全部检查后，push 分支到远程，确认后合并回 `main` 并推送。
- 不按每一个机械 Step 提交。一次 commit 必须对应一个测试通过、可以独立说明的功能切片；通常每个 Task 产生 1-3 个 commit。
- 失败测试和实现代码完成 red-green 循环后一起提交，`main` 分支上的每一个 commit 都必须能够构建并通过对应测试。
- 每个 Task 完成并通过该 Task 的全部检查后合并到 `main` 并 push 一次；每个里程碑建立一个可恢复 tag，最终建立 `v0.1.0` tag。
- 计划中每个 Task 末尾的 commit 命令是最低检查点；Task 内出现多个独立功能切片时允许增加 `test:`、`feat:` 或 `fix:` commit。
- 不使用 squash 清理课程开发历史，以便导师检查实现过程；分支合并使用普通 merge（保留分支历史）。
- `.gitignore` 必须排除 API Key、`.env`、SQLite 数据库、会议录音/转写、模型文件、`target/`、`node_modules/` 和安装包输出。
- `origin` 配置完成后，每个 Task 的最终 commit 均先 `git push -u origin feat/task-N-xxx`，确认无误后合并回 `main` 并 `git push origin main --follow-tags`；未配置远程仓库时先保留本地 commit，不阻断功能开发。

里程碑 tag 固定为：

```text
v0.1.0-m1  基础壳与音频
v0.1.0-m2  本地转写与问题检测
v0.1.0-m3  AI 回答完整闭环
v0.1.0     最终交付版本
```

---

## 4. 分阶段实施任务

### Task 1：初始化工程、测试框架与第三方登记

**Files:**
- Create: `.gitignore`, `package.json`, `vite.config.ts`, `tsconfig.json`
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`
- Create: `THIRD_PARTY_NOTICES.md`, `scripts/verify-third-party.ps1`

- [ ] **Step 1：初始化 Git 和前后端工程**

```powershell
git init
npm init -y
npm install react react-dom @tauri-apps/api
npm install -D typescript vite @vitejs/plugin-react vitest jsdom `
  @testing-library/react @testing-library/jest-dom @tauri-apps/cli
```

Expected: `package-lock.json` 生成，`npm ls --depth=0` 无 missing dependency。

`.gitignore` 必须在首次 commit 前生成，并覆盖 3.1 节列出的本地数据、大模型和密钥文件。

- [ ] **Step 2：创建 Tauri 2 Rust 工程并锁定依赖**

`src-tauri/Cargo.toml` 至少声明：`tauri`、`tokio`、`serde`、`serde_json`、`thiserror`、`tracing`、`windows`、`rusqlite`、`reqwest`、`keyring`、`whisper-rs` 和 `ort`。启用的 feature 只包含 Windows、TLS、SQLite bundled、流式 HTTP 和 Vulkan ASR 所需能力。

Run:

```powershell
cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps
npm run build
```

Expected: 两条命令退出码均为 0。

- [ ] **Step 3：实现许可证检查脚本**

脚本读取 `THIRD_PARTY_NOTICES.md` 的项目表，验证每一项包含 URL、许可证、固定版本或 commit、复用板块和修改说明。空字段使脚本退出 1。

Run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify-third-party.ps1
```

Expected: 所有已引入依赖都有登记，输出 `Third-party manifest OK`。

- [ ] **Step 4：提交基础工程**

```powershell
git add package.json package-lock.json vite.config.ts tsconfig.json src-tauri THIRD_PARTY_NOTICES.md scripts
git commit -m "chore: initialize tauri meeting assistant"
```

### Task 2：建立领域类型、Tauri 命令和双窗口 UI 骨架

**Files:**
- Create: `src/types/domain.ts`, `src/lib/events.ts`, `src/lib/tauri.ts`
- Create: `src/features/meeting/OverlayPage.tsx`, `src/components/CaptureIndicator.tsx`
- Create: `src-tauri/src/state.rs`, `src-tauri/src/commands.rs`
- Test: `src/features/meeting/OverlayPage.test.tsx`, Rust module tests

- [ ] **Step 1：先写 UI 状态测试**

```tsx
it("always shows AI and capture state", () => {
  render(<OverlayPage initialState="capturing" />);
  expect(screen.getByText("AI 辅助运行中")).toBeVisible();
  expect(screen.getByText("系统音频采集中")).toBeVisible();
});
```

Run: `npm test -- --run src/features/meeting/OverlayPage.test.tsx`

Expected: FAIL，因为 `OverlayPage` 尚未实现。

- [ ] **Step 2：实现主窗口和置顶会议面板**

`tauri.conf.json` 定义 `main` 与 `overlay` 两个窗口。`overlay` 可移动、可缩放、始终置顶，最小宽度 360px，透明度不得低于 70%，标题栏持续显示 AI 与采集状态。

Run: `npm test -- --run src/features/meeting/OverlayPage.test.tsx`

Expected: PASS。

- [ ] **Step 3：定义 Rust 会话状态机**

```rust
pub enum SessionState {
    Idle,
    Starting,
    Capturing,
    Stopping,
    Failed { message: String },
}
```

只允许 `Idle -> Starting -> Capturing -> Stopping -> Idle`；失败状态可以停止并回到 `Idle`。

Run: `cargo test --manifest-path src-tauri/Cargo.toml state::tests`

Expected: 合法迁移 PASS，非法重复启动返回 `SessionAlreadyRunning`。

- [ ] **Step 4：提交 UI 骨架**

```powershell
git add src src-tauri
git commit -m "feat: add visible meeting overlay and session state"
```

### Task 3：实现 WASAPI 系统音频和可选麦克风采集

**Files:**
- Create: `src-tauri/src/audio/wasapi.rs`, `resample.rs`, `level.rs`, `mod.rs`
- Test: 模块内单元测试和 `src-tauri/examples/loopback_probe.rs`

- [ ] **Step 1：写重采样与声道转换测试**

测试 48kHz 双声道浮点输入转换成 16kHz 单声道 `i16`；1 秒输入必须得到 16000 个样本，峰值不得溢出。

Run: `cargo test --manifest-path src-tauri/Cargo.toml audio::resample::tests`

Expected: FAIL，因为转换器尚未实现。

- [ ] **Step 2：移植并隔离 WASAPI 采集代码**

从 OneTrueDutchie Windows 的 `audio.rs` 只移植以下能力：默认 render endpoint loopback、默认 capture endpoint、格式检测、音量计算和停止信号。系统与麦克风数据写入不同 channel，不做 sample-by-sample 混合。

```rust
pub struct AudioFrame {
    pub source: AudioSource,
    pub samples_16khz_mono: Vec<i16>,
    pub captured_at_ms: u64,
}

pub enum AudioSource { System, Microphone }
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml audio::`

Expected: PASS。

- [ ] **Step 3：增加真实设备探针**

Run:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --example loopback_probe
```

Expected: 播放测试音频时 10 秒内输出 `LOOPBACK_OK`，并报告非零 RMS；没有麦克风权限时系统音频仍继续工作。

- [ ] **Step 4：提交音频模块**

```powershell
git add src-tauri/src/audio src-tauri/examples THIRD_PARTY_NOTICES.md
git commit -m "feat: capture separate system and microphone audio"
git tag v0.1.0-m1
```

### Task 4：实现模型管理、Silero VAD 和本地 Whisper 转写

**Files:**
- Create: `src-tauri/src/asr/model_manager.rs`, `whisper_worker.rs`, `mod.rs`
- Create: `src-tauri/src/vad/silero.rs`, `segmenter.rs`, `mod.rs`
- Create: `src-tauri/models/models.json`
- Test: `tests/fixtures/audio/zh_question.wav`, `en_question.wav`, `silence.wav`

- [ ] **Step 1：定义并测试模型清单校验**

`models.json` 每个模型必须包含 `id`、下载 URL、SHA-256、文件大小、语言范围和运行档位。下载写入临时文件，校验通过后原子重命名；失败文件立即删除。

Run: `cargo test --manifest-path src-tauri/Cargo.toml asr::model_manager::tests`

Expected: 错误哈希被拒绝，断点续传和校验通过场景 PASS。

- [ ] **Step 2：实现 VAD 分段状态机**

固定策略：30ms 帧、300ms 前置缓存、连续 180ms 语音开始片段、连续 600ms 静音结束片段、单段最长 25 秒。系统音频优先；麦克风队列拥塞时允许丢弃临时片段但不阻塞系统音频。

Run: `cargo test --manifest-path src-tauri/Cargo.toml vad::`

Expected: 静音不产出片段，两段语音被正确分开，超长语音在 25 秒切分。

- [ ] **Step 3：实现 Whisper 工作线程**

工作线程启动时加载高质量 `large-v3-turbo` 量化模型，Vulkan 初始化失败时自动加载 CPU 回退模型。每 800ms 对滚动窗口生成临时结果，VAD 完成后生成最终结果；只有最终结果进入问题检测。

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml asr::whisper_worker::tests
```

Expected: 中文和英文 fixture 均生成非空最终文本；静音不生成文本；`speaker` 来源正确。

- [ ] **Step 4：提交本地 ASR**

```powershell
git add src-tauri/src/asr src-tauri/src/vad src-tauri/models tests/fixtures/audio THIRD_PARTY_NOTICES.md
git commit -m "feat: add local streaming speech recognition"
```

### Task 5：实现问题检测、去重与上下文窗口

**Files:**
- Create: `src-tauri/src/question/detector.rs`, `normalizer.rs`, `mod.rs`
- Test: 模块内中英问题语料表测试

- [ ] **Step 1：写问题检测表测试**

至少覆盖：中文疑问句、英文疑问句、命令式面试题、普通陈述、重复转写和被截断的问题。

```rust
#[test]
fn recognizes_interview_prompts() {
    assert!(detect("请介绍一下你负责的项目").is_some());
    assert!(detect("What was the hardest problem you solved?").is_some());
    assert!(detect("今天天气不错").is_none());
}
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml question::`

Expected: FAIL，因为检测器尚未实现。

- [ ] **Step 2：实现轻量本地检测器**

规则覆盖 `吗/呢/为什么/怎么/如何/什么/多少/是否/能否`、英文问词和 `请介绍/谈谈/解释/举例/describe/explain/tell me`。检测器读取最近 20 秒 remote 最终文本，将相邻短句合并；标准化文本哈希在 30 秒内重复时不再次触发。

Run: `cargo test --manifest-path src-tauri/Cargo.toml question::`

Expected: 全部表测试 PASS。

- [ ] **Step 3：增加置信度和人工触发**

置信度不低于 0.65 时自动生成；0.40-0.64 时只显示“可能的问题”，用户点击生成；低于 0.40 忽略。全局快捷键只能将最近 20 秒转写作为问题手动提交，不执行自动输入。

- [ ] **Step 4：提交问题检测模块**

```powershell
git add src-tauri/src/question
git commit -m "feat: detect and deduplicate spoken questions"
git tag v0.1.0-m2
```

### Task 6：实现资料导入与本地相关片段匹配

**Files:**
- Create: `src-tauri/src/profile/importer.rs`, `extractor.rs`, `matcher.rs`, `mod.rs`
- Create: `src/features/profile/ProfileLibraryPage.tsx`
- Test: `tests/fixtures/documents/sample.pdf`, `sample.docx`, `sample.md`

- [ ] **Step 1：写文档解析和安全测试**

测试正常 PDF/DOCX/TXT/MD、空文件、超过 5MB 文件、损坏 ZIP、包含外部关系的 DOCX。解析器不得访问文档中的 URL 或外部资源。

Run: `cargo test --manifest-path src-tauri/Cargo.toml profile::extractor::tests`

Expected: FAIL，因为解析器尚未实现。

- [ ] **Step 2：实现本地解析和规范化**

每份文档最多 5MB，最多导入 10 份。去除重复空白和控制字符，保留标题与段落；原文件路径和提取文本存本机，不上传原文件。

- [ ] **Step 3：实现确定性的关键词匹配**

按 400-800 字符切块，保留 80 字符重叠；使用中英文 token、标题加权和 BM25 风格分数，选出最多 4 个片段且总长度不超过 6000 字符。当前会议可选择最多 3 份启用资料。

Run: `cargo test --manifest-path src-tauri/Cargo.toml profile::`

Expected: 与“音频延迟优化”相关的问题优先命中包含 WASAPI、VAD、Whisper 的项目片段。

- [ ] **Step 4：完成资料库 UI 并提交**

```powershell
npm test -- --run src/features/profile
git add src/features/profile src-tauri/src/profile tests/fixtures/documents
git commit -m "feat: add local profile library and context matching"
```

### Task 7：实现 DeepSeek、OpenAI 和自定义流式答案适配器

**Files:**
- Create: `src-tauri/src/answer/provider.rs`, `deepseek.rs`, `openai.rs`, `compatible.rs`, `prompt.rs`, `mod.rs`
- Test: provider mock server 集成测试

- [ ] **Step 1：定义统一 provider 接口**

```rust
pub struct AnswerRequest {
    pub question_id: String,
    pub question: String,
    pub recent_transcript: Vec<String>,
    pub profile_context: Vec<String>,
    pub response_language: String,
}

pub enum AnswerEvent {
    Started,
    ShortAnswerDelta(String),
    KeyPoints(Vec<String>),
    FollowUps(Vec<String>),
    Completed,
    Failed(String),
}
```

所有 provider 必须支持 15 秒连接超时、60 秒总超时、用户取消和不超过一次的网络重试；认证失败和限流不自动重试。

- [ ] **Step 2：先写流式协议测试**

Mock server 分别返回正常 SSE、多字节 UTF-8 被拆包、HTTP 401、HTTP 429、中途断线和用户取消。测试必须证明 delta 顺序正确且取消后不再发送 UI 事件。

Run: `cargo test --manifest-path src-tauri/Cargo.toml answer::`

Expected: FAIL，因为 adapter 尚未实现。

- [ ] **Step 3：实现 provider adapter**

- DeepSeek 与 Custom 使用 OpenAI-compatible chat streaming wire format。
- OpenAI 使用当前官方 Responses API 的流式事件；具体模型 ID 由设置页保存，不在代码中写死“最新模型”。
- Custom 提供“本地兼容服务”预设，默认地址为 `http://127.0.0.1:11434/v1`；它只连接已经由用户启动的 Ollama，不检测、安装或管理 Ollama 和本地答案模型。
- Prompt 将会议转写和导入资料标记为不可信数据，禁止其中的文字覆盖系统规则。
- 输出顺序固定为 `short_answer -> key_points -> follow_ups`；格式异常时保留已收到的短答，并将后续内容降级成普通要点。

Run: `cargo test --manifest-path src-tauri/Cargo.toml answer::`

Expected: 所有 mock 场景 PASS。

- [ ] **Step 4：提交答案适配器**

```powershell
git add src-tauri/src/answer
git commit -m "feat: stream answers from configurable AI providers"
```

### Task 8：实现安全设置、SQLite 历史和 7 天保留策略

**Files:**
- Create: `src-tauri/migrations/001_initial.sql`
- Create: `src-tauri/src/storage/database.rs`, `credentials.rs`, `retention.rs`, `mod.rs`
- Create: `src/features/settings/SettingsPage.tsx`
- Test: storage module tests and settings UI tests

- [ ] **Step 1：写数据库迁移和保留策略测试**

数据库表固定为 `meetings`、`transcript_segments`、`questions`、`answers`、`profile_documents` 和 `settings`。测试覆盖新库迁移、事务回滚、固定记录保留、8 天旧记录删除和关联记录级联删除。

Run: `cargo test --manifest-path src-tauri/Cargo.toml storage::`

Expected: FAIL，因为 migration 和 repository 尚未实现。

- [ ] **Step 2：实现数据库与凭据存储**

SQLite 保存文本和元数据；API Key 仅写入 Windows Credential Manager，SQLite 保存 `credential_id`。日志不得输出 key、完整资料内容或完整答案请求体。

- [ ] **Step 3：实现自动清理和立即删除**

应用启动后执行一次清理，运行期间每 24 小时执行一次。删除操作使用单个事务，并删除对应的提取文本；用户选择“清除全部数据”时先显示明确确认对话框。

Run: `cargo test --manifest-path src-tauri/Cargo.toml storage::`

Expected: 全部测试 PASS。

- [ ] **Step 4：实现设置页并提交**

设置页提供 provider、base URL、model、API Key 更新、连接测试、ASR 模型选择、麦克风开关和保留天数。API Key 输入框永不回显已有明文。

```powershell
npm test -- --run src/features/settings
git add src/features/settings src-tauri/src/storage src-tauri/migrations
git commit -m "feat: secure credentials and local data retention"
```

### Task 9：连接完整会话流水线和会议面板交互

**Files:**
- Modify: `src-tauri/src/commands.rs`, `state.rs`, `lib.rs`
- Create: `src/features/meeting/MeetingPage.tsx`
- Modify: `src/features/meeting/OverlayPage.tsx`
- Create: `src/components/AudioMeters.tsx`, `TranscriptFeed.tsx`, `AnswerCard.tsx`
- Test: frontend integration tests and Rust orchestrator tests

- [ ] **Step 1：写编排测试**

使用假的 AudioSource、ASR、QuestionDetector、ProfileMatcher 和 AnswerProvider，断言事件顺序：

```text
capture-state
audio-level
transcript-pending
transcript-final
question-detected
answer-started
answer-delta
answer-completed
```

新问题到来时，如果旧答案仍在生成且未被用户固定，则取消旧请求并生成最新问题；用户固定旧答案后，新问题进入单项等待队列。

Run: `cargo test --manifest-path src-tauri/Cargo.toml commands::tests`

Expected: FAIL，因为完整编排尚未连接。

- [ ] **Step 2：连接后端流水线**

开始会话时依次检查模型、音频设备和 provider 配置；任一步失败都回滚已启动资源。停止会话必须在 2 秒内取消音频、ASR 和网络请求并提交数据库事务。

- [ ] **Step 3：完成会议 UI**

面板固定展示 AI/采集状态，正文显示识别问题和流式短答；要点和追问默认折叠。按钮包括取消、重新生成、复制、固定记录、字体调整和停止会话。所有图标使用 Lucide，并提供 tooltip 与键盘焦点样式。

Run:

```powershell
npm test -- --run src/features/meeting src/components
cargo test --manifest-path src-tauri/Cargo.toml commands::tests
```

Expected: 全部 PASS。

- [ ] **Step 4：提交完整会话闭环**

```powershell
git add src src-tauri/src
git commit -m "feat: connect live meeting assistance pipeline"
git tag v0.1.0-m3
```

### Task 10：性能基准、故障恢复和真实会议验收

> 说明：本 Task 基于 Task 9 已交付的编排实现（`session.rs` Orchestrator + `pipeline.rs` RealPipeline + 前端对话式 UI/置顶窗）。
> 先清理 Task 9 期间遗留的临时诊断日志（`[pipeline]`/`[orchestrator]`/`[session]` eprintln），再接入正式时间戳。

**Files:**
- Modify: `src-tauri/src/session.rs`, `src-tauri/src/pipeline.rs`（清理临时诊断日志，接入正式时间戳与结构化日志）
- Create: `src-tauri/benches/pipeline_latency.rs`
- Create: `docs/test-cases.md`, `docs/privacy-and-consent.md`
- Create: `tests/manual/tencent-meeting-checklist.md`
- Modify: `src-tauri/src/session.rs`（测试：FakeProvider 支持注入首包延迟）

- [ ] **Step 1：清理临时诊断日志并接入端到端时间戳**

删除 Task 9 调试期加入的 `eprintln!("[pipeline] ...")`、`[orchestrator]`、`[session]` 等临时代码（含 `last_level_log`、`PROB_LOG` 静态计数、`[diag]` 契约打印），正式计时点改为 `tracing` 事件并只记录耗时、状态与 ID，不记录敏感正文。

计时点：`speech_started`、`speech_ended`、`transcript_final`、`question_detected`、`provider_connected`（收到首个 SSE data）、`first_answer_delta`。每个点携带 `duration_ms` 与关联 ID；`session.rs` 测试继续通过。

- [ ] **Step 2：运行离线性能基准**

`pipeline_latency.rs` 基准按 Task 9 组件拆分：
- 本地问题检测：`question::detector` 直接基准（无需模型），断言 P95 ≤ 50ms。
- 端到端转写：读取 `tests/fixtures/audio/zh_question.wav`，经 `vad::segmenter`（FakeVad）+ `asr::whisper_worker` 真实模型转写；**依赖用户本地已导入模型**，模型缺失时基准打印跳过说明（不视为失败）。

```powershell
cargo bench --manifest-path src-tauri/Cargo.toml --bench pipeline_latency
```

Expected: 在 i7-10700K + RTX 3060 Ti 8GB 上，fixture 的最终转写 P50 不超过语音结束后 1.5 秒；本地问题检测 P95 不超过 50ms。

- [ ] **Step 3：运行模拟 API 延迟测试**

扩展 `session.rs` 测试的 FakeProvider：支持注入 100ms / 500ms / 1500ms 首包延迟与中途断流。断言：事件顺序不变、`cancel` 后 200ms 内不再有新事件（取消即时生效）、停止会话 2 秒内任务结束。前端 `MeetingPage.test.tsx` 补一条：流式期间 UI 保持可交互（按钮可用）。

网络断开场景：断流后转写与问题气泡保留（前端不因 `answer-failed` 清空会话），答案卡显示失败状态并可「重新生成」——沿用 Task 9 已实现的 `Failed` 事件与重试按钮，测试覆盖失败状态渲染。

- [ ] **Step 4：执行腾讯会议人工验收**

由导师/测试参与者知情后，按 `tests/manual/tencent-meeting-checklist.md` 逐项验证（清单按已交付 UI 编写）：
- 采集与状态：主界面「开始会话」→ AI 状态/采集指示/音量表实时变化；停止会话 2 秒内回到待机；置顶小窗口同步显示最新问题与流式短答。
- 语言场景：中文问题、英文问题、中英混说、长问题（25 秒切分）、对方打断（新问题取消未固定旧答案 / 固定后排队）。
- 设备与网络：扬声器与耳机输出、麦克风开启/关闭（设置页开关）、切换默认播放设备（重新开始会话）、断网恢复（保留转写，答案卡失败可重试）、API 401/429（认证失败/限流不重试，错误提示跳转设置页）。
- 稳定性与清理：运行 60 分钟无崩溃；结束后检查 7 天保留策略与「清除全部数据」。

- [ ] **Step 5：提交性能与测试文档**

```powershell
git add src-tauri/benches docs tests/manual src-tauri/src/session.rs src-tauri/src/pipeline.rs
git commit -m "test: add latency and real meeting acceptance suite"
```

### Task 11：Windows 安装包、模型导入体验与交付文档

> 说明：按用户约定 **v0.1.0 不做模型自动下载**——模型由用户本地导入（`asr/model_manager.rs` 的 `import_model` 计算并登记 SHA-256，清单 `src-tauri/models/models.json` 只提供元数据与校验信息）。
> 本 Task 的"模型体验"指：设置页提供模型导入/校验入口 + 首次启动引导说明，而非程序自动下载。

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/models/models.json`（补全 silero 模型条目与已导入模型校验记录）
- Modify: `src/features/settings/SettingsPage.tsx`（新增模型导入/校验入口；现有 ASR 模型下拉改为展示本地已导入模型）
- Modify: `src-tauri/src/commands.rs`（新增模型导入/列表/校验命令，复用 `asr::model_manager`）
- Create: `README.md`, `docs/build.md`, `docs/architecture.md`, `docs/troubleshooting.md`
- Modify: `THIRD_PARTY_NOTICES.md`

- [ ] **Step 1：配置 NSIS 安装包与模型导入体验**

安装包只包含应用本体，不内置 GB 级模型。卸载时默认保留用户数据，并提供“同时删除本地数据”的明确选项。

模型导入体验（**不自动下载**）：
- 首次启动：设置页显示模型状态卡片（未导入 / 已导入 + SHA-256 校验结果），说明模型来源（whisper.cpp 官方 Release）与体积（large-v3-turbo q5_0 ≈ 574MB；silero_vad.onnx ≈ 2.3MB）。
- 用户通过「导入模型文件」选择本地已下载的 `.bin`/`.onnx` 文件；`ModelManager::import_model` 校验 SHA-256 后原子替换到 `%LOCALAPPDATA%\MeetingAIAssistant\models\` 并登记。
- 校验失败/文件损坏：明确报错并允许重新选择（沿用 Task 6 教训：损坏的 404 HTML 文件会被 protobuf/哈希校验拦截）。
- 设置页 ASR 模型下拉只列出已导入模型；Vulkan 初始化失败时自动回退 CPU（Task 4 已实现，文档说明延迟可能增加）。
- Ollama/LM Studio 不随安装包分发。

- [ ] **Step 2：编写可复现构建文档**

文档列出 Node 22、Rust stable MSVC、Visual Studio Build Tools、Windows SDK、CMake 和测试命令；使用 `package-lock.json` 与 `Cargo.lock` 保证依赖可复现。

`docs/architecture.md` 按已交付模块编写：`audio/`（WASAPI 双路采集）、`vad/`（Silero v6 契约：512 样本帧、input/state/sr）、`asr/`（Vulkan→CPU 回退、静音门控）、`question/`（20s 窗口 + 30s 去重 + 前缀续接抑制）、`profile/`（本地解析与 BM25 匹配）、`answer/`（OpenAI-compatible SSE、超时/重试/取消、三段输出）、`storage/`（SQLite 六表 + Credential Manager + 7 天保留）、`session.rs` 编排（取消/排队竞争策略、事件契约）、前端（对话式 UI、置顶窗、`useSessionEvents`）。

`docs/troubleshooting.md` 收录已知问题与排查：模型文件损坏（校验失败）、silero 模型契约版本差异、WASAPI 默认设备切换、多显卡 Vulkan 设备枚举（`GGML_VK_VISIBLE_DEVICES=0`）、GitHub/网络下载失败、凭据管理器不可用。

- [ ] **Step 3：完成第三方许可证审计**

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify-third-party.ps1
cargo deny check licenses --manifest-path src-tauri/Cargo.toml
```

Expected: 没有未登记、许可证不兼容或来源未知的依赖。

- [ ] **Step 4：清理历史警告并运行最终质量门禁**

先处理历史遗留：移除不再使用的 `#[allow(dead_code)]`（如 `state.rs` 的 Failed 变体若已使用则删除标注）、未使用的 `now_ms`/辅助项，运行 `cargo fmt` 统一格式；随后执行：

```powershell
npm test -- --run
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

Expected: 所有测试、静态检查和构建通过；生成 Windows NSIS 安装包。

- [ ] **Step 5：提交可交付版本**

```powershell
git add README.md docs THIRD_PARTY_NOTICES.md src-tauri/tauri.conf.json src-tauri/models package-lock.json src-tauri/Cargo.lock
git commit -m "docs: prepare audited Windows prototype delivery"
git tag v0.1.0
```

---

## 5. 错误处理与降级规则

| 故障 | 程序行为 |
|---|---|
| 默认播放设备变化 | 停止旧 WASAPI client，重新绑定新设备；面板显示“正在重连音频” |
| 麦克风无权限 | 禁用本地音轨并继续系统音频，不阻断会议 |
| GPU/Vulkan 初始化失败 | 加载 CPU 回退模型并提示延迟可能增加 |
| 模型不存在或损坏 | 禁止开始会话，进入可重试的下载/校验页面 |
| API Key 无效 | 保留转写，答案卡显示认证错误并跳转设置页 |
| API 限流 | 不自动连续重试；显示等待时间并允许切换 provider |
| 网络中断 | 继续本地转写和问题记录，恢复后由用户重新生成答案 |
| 新问题覆盖旧问题 | 取消未固定的旧生成；已固定答案保留并将新问题排队 |
| 数据库写入失败 | 会话继续运行，内存暂存当前数据并显示“本地保存失败” |
| 应用退出 | 先停止采集和网络请求，再刷新数据库；超时 2 秒后强制释放资源 |

---

## 6. 最终交付物

- Windows 11 x64 NSIS 安装包和 portable 构建产物。
- 完整源代码、`package-lock.json`、`Cargo.lock` 和模型清单。
- `README.md`：功能、安装、首次配置和使用说明。
- `docs/architecture.md`：音频、ASR、问题检测、资料匹配和答案流数据图。
- `docs/test-cases.md`：自动化测试与性能结果。
- `docs/privacy-and-consent.md`：采集范围、本地数据、API 发送内容和知情测试流程。
- `THIRD_PARTY_NOTICES.md`：全部开源项目、commit、许可证、复用文件和修改记录。
- 课程演示脚本：启动程序、导入资料、接入腾讯会议、提出中英文问题、展示流式答案、断网恢复和清除记录。

## 7. 实施顺序与里程碑

1. **M1 基础壳与音频：** Task 1-3，能在 UI 中看到腾讯会议系统音量。
2. **M2 本地转写：** Task 4-5，能实时显示问题文本且不调用云端 ASR。
3. **M3 AI 回答闭环：** Task 6-9，能结合资料流式显示结构化答案。
4. **M4 质量与交付：** Task 10-11，满足性能、稳定性、许可证和安装包要求。

每个里程碑必须先通过对应自动化测试和人工验收，再进入下一阶段；不得为了演示跳过密钥保护、采集状态显示或第三方许可证登记。
