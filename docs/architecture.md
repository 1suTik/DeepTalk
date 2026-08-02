# 架构文档

## 总览

Tauri 2 双窗口桌面应用：主窗口为完整对话界面，置顶小窗口为精简会议面板；Rust 后端完成音频采集、本地转写、问题检测、资料匹配与流式答案，通过稳定事件契约向前端推送领域数据。

```text
WASAPI 采集（系统 loopback / 麦克风，独立 channel）
   │ AudioFrame(16kHz mono i16)
   ▼
pipeline.rs：拼帧(512样本/32ms) → Silero VAD v6 → 分段(180ms起/600ms止/25s上限)
   │ PipelineEvent（capture-state / audio-level / transcript-pending / transcript-final）
   ▼
session.rs Orchestrator：问题检测 → 资料匹配 → 答案生成（取消/排队竞争策略）
   │ OrchestrationEvent → TauriSink → tauri emit
   ▼
前端 useSessionEvents → 对话 UI（主窗口）/ 精简面板（置顶窗口）
```

## 模块职责

### 音频 `audio/`
- `wasapi.rs`：WASAPI 共享模式采集；loopback 打开默认 render 端点，麦克风打开默认 capture 端点；`AUDCLNT_STREAMFLAGS_LOOPBACK` + 静音包补零；每个采集线程独立 COM 公寓。
- `resample.rs`：任意输入格式 → 16kHz 单声道 i16（线性重采样 + 峰值钳制）。
- `level.rs`：RMS / peak 音量计算。

### VAD `vad/`
- `silero.rs`：Silero VAD **v6 契约**（`input/state/sr` → `output/stateN`；state=[2,1,128]；输入必须精确 512 样本 = 32ms @16kHz）。v5 契约（input/sr/h/c）不兼容。
- `segmenter.rs`：分段状态机（30ms 帧基准计数；300ms 前置缓存、连续 180ms 语音起段、600ms 静音收段、单段最长 25 秒；段不含尾部静音）。与具体 VAD 解耦（测试用 FakeVad）。

### ASR `asr/`
- `whisper_worker.rs`：whisper.cpp 绑定；Vulkan 初始化失败自动回退 CPU；RMS < 0.005 静音门控 + `no_speech ≥ 0.6` 过滤（防「Thank you.」幻觉）；**全局 GPU_LOCK 串行化**（多 GPU 上下文访问违规）；默认 `GGML_VK_VISIBLE_DEVICES=0`（多显卡虚拟适配器崩溃）。
- `model_manager.rs`：模型清单校验、本地导入（SHA-256 + 原子替换）、注册表；**v0.1.0 不做自动下载**。

### 问题检测 `question/`
- `detector.rs`：20 秒窗口 + 相邻短句合并；规则置信度（Auto ≥ 0.65 / Maybe 0.40-0.64 / 忽略 < 0.40）；标准化哈希 30 秒去重 + **前缀续接抑制**（转写分片导致同一句触发两次）。
- `normalizer.rs`：文本规范化（去标点/空白，统一大小写）。

### 资料 `profile/`
- `extractor.rs`：PDF(lopdf)/DOCX(zip+quick-xml)/TXT/MD 本地解析；≤5MB、PDF≤500 页、DOCX≤200 条目且解压≤20MB；**不访问文档内 URL/外部资源**。
- `importer.rs`：最多 10 份、同路径去重、启停、本地 JSON 存储。
- `matcher.rs`：400-800 字符切块（80 重叠）+ 中英文 token + 标题 1.5× 加权 + BM25 风格评分；≤4 片段且 ≤6000 字符；单场会议最多取 3 份启用资料。

### 答案 `answer/`
- `provider.rs`：统一 `AnswerProvider` trait（BoxFuture 支持 dyn 注入）；OpenAI-compatible chat streaming（DeepSeek/Custom）与 OpenAI Responses API 两种 wire format；连接 15s / 总 60s 超时；网络错误最多重试 1 次（401/403 认证失败与 429 限流不重试）；取消令牌（tokio Notify 实现）；SSE 解析容错（UTF-8 跨分块、格式异常降级为普通要点）。
- `deepseek.rs` / `openai.rs` / `compatible.rs`：三种提供商（Custom 默认 `http://127.0.0.1:11434/v1`，只连接用户已启动的 Ollama/LM Studio，不检测/安装/管理）。
- `prompt.rs`：不可信数据防护（转写/资料/问题中的提示注入指令一律无视）；**资料为辅助参考，未命中时基于模型知识正常回答**；固定三段输出标记 `[短答]/[要点]/[追问]`；短答按 token 流原样拼接（空行才分段）。

### 存储 `storage/`
- `database.rs`：SQLite（bundled），六表 `meetings/transcript_segments/questions/answers/profile_documents/settings`；外键级联；批量插入事务回滚；`Arc<Mutex<Connection>>` 共享。
- `credentials.rs`：API Key 仅存 Windows Credential Manager（keyring），SQLite 只存引用。
- `retention.rs`：默认 7 天（可配）；只删未固定且已结束的会议；启动即清 + 每 24h 后台线程（**std 线程，勿用 tokio::spawn**——AppState 初始化早于 tauri runtime）。

### 编排 `session.rs`
- `Orchestrator`：消费 `PipelineEvent`，产出 `OrchestrationEvent`；问题只消费 system（remote）转写；答案请求携带最近 10 条转写 + 命中的资料片段。
- 竞争策略：新问题到来时**取消未固定的旧答案**；用户固定后新问题进入**单项等待队列**；Maybe 问题仅提示，用户点击生成。
- `EventSink` trait：测试用 channel，生产用 `TauriSink`（app.emit 稳定事件）。
- 停止：取消答案 → 关闭流水线 → 结束会议 → 采集结束事件，2 秒内完成。

### 前端
- `lib/events.ts`：事件契约（8 个事件 + payload 类型）；`lib/tauri.ts`：invoke 包装 + 非 Tauri 环境降级。
- `features/meeting/useSessionEvents.ts`：事件订阅 hook（主窗口与置顶窗口共用）。
- `features/meeting/MeetingPage.tsx`：对话式主界面（问题气泡 + 答案卡）；`OverlayPage.tsx`：置顶精简面板。
- `features/settings/SettingsPage.tsx`：provider/API Key（永不回显）/连接测试/模型导入/保留天数/清除数据。

## 数据流时序（一次完整问答）

```text
capture-state → audio-level* → transcript-pending* → transcript-final
  → question-detected → answer-started → answer-delta* → answer-completed
```

## 时间戳（tracing）

`speech_started` / `speech_ended` / `transcript_final`（pipeline.rs）、`question_detected`（session.rs）、`provider_connected` / `first_answer_delta`（session.rs 答案任务）；只记录耗时、状态与 ID，不记录敏感正文。
