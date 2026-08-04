# DeepTalk（对话助手） · v0.2.0

![DeepTalk Logo](src-tauri/icons/icon.png)

Windows 实时对话 AI 辅助程序：本地完成中英语音转写，识别提问，并通过 DeepSeek / OpenAI / 自定义 OpenAI 兼容接口流式显示建议答案。

## 功能特性

- **本地语音转写**：WASAPI 采集系统音频（loopback）与可选麦克风，Silero VAD 分段，Whisper（Vulkan，失败自动回退 CPU）本地转写；中英双语自动识别，音频不出本机。
- **问题检测**：20 秒窗口识别完整问题后自动触发生成答案，30 秒去重与分片续接抑制；低置信度问题提供手动生成。
- **流式答案**：DeepSeek / OpenAI / 自定义 OpenAI 兼容服务；SSE 流式输出，固定「短答 → 要点 → 追问」三段结构；支持取消、重新生成、复制、固定记录（固定后新问题排队）。
- **提示词方案切换**：内置「面试助手」「通用助手」两套预设，支持在设置页新建 / 编辑 / 删除自定义方案（系统提示词 + 用户提示词模板），切换后下一轮答案立即生效；内置提示词内置防提示注入规则，转写与资料视为不可信数据。
- **置顶小窗**：无边框置顶小窗实时显示最新问题与流式短答，可由主界面随时打开 / 关闭；小窗关闭（隐藏）后不销毁，随时可重新唤起。
- **本地资料库**：导入 PDF / DOCX / TXT / Markdown 简历与岗位说明，答案结合命中的资料片段；资料只在本机解析与检索。
- **隐私与保留**：音频不出本机；API Key 存入 Windows Credential Manager（永不落库、永不回显）；对话历史本地 SQLite 存储，默认 7 天自动清理未固定记录。

## UI 设计系统（v0.2.0 全新）

v0.2.0 将全套界面升级为「晨光白 × 克制玻璃」设计系统，主界面、设置页与置顶小窗三处统一同源，彻底告别深蓝黑底 + 紫色渐变的“AI 大众脸”。

- **晨光白主题**：暖纸白底色（`#f4f1eb`）+ 珊瑚红强调色（`#dd5a47`），墨色正文、柔和状态色（墨绿 / 琥珀 / 珊瑚红），浅色主题下长时间使用更舒适。
- **克制液态玻璃**：半透明玻璃卡片 + 1px 细描边 + 柔和阴影，背景带极淡的暖珊瑚与浅蓝光晕；大圆角（18–24px），只模糊应用内背景，不依赖系统级毛玻璃，Win10 / Win11 表现一致。
- **交互动画**：悬停上浮、按钮按压回弹、消息入场淡入上移、流式答案光标闪烁；全部 160–220ms 缓动，并支持系统「减少动态效果」设置自动降级。
- **鼠标光斑**：主窗口背景有一团极淡的珊瑚色光斑跟随指针（rAF 节流，`pointer-events: none`，不拦截任何操作），置顶小窗不启用以保证简洁。
- **设计令牌**：颜色、圆角、阴影、动效统一由 CSS 变量管理（`src/styles.css` 的 `:root`），后续改主题只动一处。
- **新品牌图标**：红 / 粉鲸鱼 + Wi-Fi 弧线 Logo 替换原 M 图标，已覆盖窗口、任务栏、安装包、iOS 与 Android 全套尺寸。

## 技术栈与依赖

| 领域 | 技术 | 说明 |
|---|---|---|
| 桌面外壳 | Tauri 2（Rust + WebView2） | 主窗口 / 置顶小窗 / 命令与事件 / NSIS 打包 |
| 前端 | React 19 + TypeScript + Vite | 界面、状态与构建 |
| 音频采集 | Windows WASAPI（windows-rs） | 系统 loopback + 麦克风，独立声道不混合 |
| 语音活动检测 | Silero VAD v6（ONNX Runtime，pykeio/ort） | 30ms 帧级起止点检测 |
| 语音转写 | whisper.cpp（whisper-rs，GGML Vulkan / CPU） | 16kHz 单声道本地推理 |
| 答案服务 | DeepSeek / OpenAI / 自定义兼容（reqwest SSE） | OpenAI-compatible chat / Responses 流式协议 |
| 本地存储 | SQLite（rusqlite，bundled）+ Windows Credential Manager（keyring） | 历史记录与凭据 |
| 文档解析 | lopdf、zip2 + quick-xml | PDF / DOCX / TXT / Markdown |
| 测试 | vitest + testing-library（前端）、Rust 单元测试（后端） | 全量门禁：cargo test / clippy / fmt + npm test / tsc / build |

完整第三方清单与许可证见 [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)。

## 模型（本地导入，不做自动下载）

| 模型 | 用途 | 体积 | 来源 |
|---|---|---|---|
| Whisper large-v3-turbo (Q5_0) | 中英转写 | ≈ 574 MB | whisper.cpp 官方 Release（HuggingFace） |
| Whisper base (Q5_0) | CPU 回退 | ≈ 61 MB | 同上 |
| Silero VAD v6 (16k ONNX) | 语音活动检测 | ≈ 2.3 MB | snakers4/silero-vad 官方仓库 |

将下载好的模型文件放入 `%LOCALAPPDATA%\MeetingAIAssistant\models\`，在设置页点击「扫描并校验」；SHA-256 校验通过后自动登记。校验失败（文件损坏）会被拒绝并提示重新选择。

## 安装与使用

### 安装包（推荐）

从 [Releases](https://github.com/1suTik/DeepTalk/releases) 下载 `DeepTalk_0.2.0_x64-setup.exe`，双击安装（当前用户安装，无需管理员权限）。

首次使用：

1. 设置页配置答案服务（DeepSeek / OpenAI / 自定义 OpenAI 兼容）与 API Key，点击「连接测试」。
2. 导入语音模型（见上节模型导入）。
3. 主界面点击「开始会话」，播放对话音频即可；识别到问题后自动生成答案。

### 从源码构建

开发环境要求：

- Node.js 20+（前端构建）
- Rust 1.88+（stable MSVC 工具链）
- Windows 10/11 + VS 2022 Build Tools（含 C++ 工具链）
- Vulkan SDK 1.4.350.0（whisper-rs vulkan feature 编译期需要头文件）
- CMake 与 Ninja（whisper.cpp 构建）

```powershell
npm install
npm run tauri dev        # 开发模式
npm run tauri build      # 生成 Windows NSIS 安装包
```

## 常用命令

```powershell
npm test -- --run                                             # 前端测试
npm run build                                                 # 前端构建
cargo test --manifest-path src-tauri/Cargo.toml               # Rust 测试（模型依赖用例为 #[ignore]）
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored asr::whisper_worker  # 真实模型转写用例
```

## 项目结构

```
src/                前端（React + TypeScript）
  app/              应用外壳与窗口路由（主界面 / 设置 / 置顶小窗）
  features/         功能页面（主界面对话流、设置页、小窗）
  lib/              Tauri 命令封装与事件订阅
  types/            前后端领域契约
src-tauri/          Rust 后端
  src/audio/        WASAPI 采集与 16kHz 重采样
  src/vad/          Silero VAD 分段
  src/asr/          Whisper 转写与模型管理（清单校验 / 本地导入）
  src/question/     问题检测与归一化
  src/answer/       答案 Provider（DeepSeek / OpenAI / 兼容）与提示词方案
  src/session/      会话状态机与流水线编排
  src/storage/      SQLite 历史与凭据管理
  migrations/       SQLite 迁移
  models/           模型清单（models.json）
```

## 数据与隐私

- 音频与转写仅在本机处理，不离开设备。
- API Key 仅存于 Windows Credential Manager，数据库中不保存任何密钥。
- 对话历史存于本机 SQLite（`%LOCALAPPDATA%\MeetingAIAssistant\history.db`），设置页可一键清除全部数据。

## 许可证

MIT。第三方依赖清单与许可证见 [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)。
