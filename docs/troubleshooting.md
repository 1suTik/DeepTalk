# 常见问题排查

## 模型相关

### 「未找到本地 Whisper 模型」
模型未导入或不在预期目录。将模型文件放入 `%LOCALAPPDATA%\MeetingAIAssistant\models\`，在设置页点击「扫描并校验」。

### 导入校验失败（SHA-256 不匹配）
文件损坏或版本不一致。**已知教训**：曾因下载到 404 HTML 页面（伪装成 `.onnx`，约 318KB）导致 Protobuf 解析失败——校验失败时请从官方来源重新下载：
- Whisper：https://huggingface.co/ggml-org/whisper.cpp
- Silero VAD v6：`snakers4/silero-vad` 仓库 `src/silero_vad/data/silero_vad.onnx`（约 2.3MB，注意 **不是** 仓库根目录的旧路径）

### Silero 模型契约版本不兼容
v5 契约 `input/sr/h/c`（state 64）与 v6 契约 `input/state/sr`（state=[2,1,128]，输入必须精确 512 样本）不兼容。本应用按 **v6** 适配；若误放旧版模型，VAD 概率恒为 0（无转写）——重新导入 v6 模型。

## 音频相关

### 音量表无数据 / 无转写
1. 检查设置页模型状态：Whisper 与 Silero 都必须「已导入 ✓」。
2. 确认默认播放设备有声音输出（WASAPI loopback 采集的是默认 render 设备；静音时不产生数据包）。
3. 切换默认播放设备后需停止并重新开始会话。

### 麦克风不可用
麦克风采集失败不阻断系统音频（降级运行）。检查设置页「启用麦克风转写」开关与 Windows 麦克风隐私权限。

## Vulkan / GPU 相关

### 启动时崩溃或 Vulkan 初始化失败
- 多显卡机器（含虚拟显示适配器，如远程桌面类软件）上 ggml-vulkan 枚举设备可能崩溃：确保 `GGML_VK_VISIBLE_DEVICES=0`（程序默认设置，不要手动覆盖为其他值）。
- 显存不足时 whisper.cpp 会回退 CPU（延迟增加属正常）。
- 转写慢：确认 NVIDIA 驱动已更新，且程序运行于 Vulkan 后端（日志 `use gpu = 1`）。

## 网络与答案服务

### 连接测试失败
- 401：API Key 无效——设置页重新输入并保存（Key 存 Windows 凭据管理器）。
- 429：限流——等待 Retry-After 后重试，不自动重试。
- 超时/断流：网络问题；答案卡显示失败状态，可点「重新生成」。
- Custom provider：确认本机 Ollama/LM Studio 已启动并监听 `127.0.0.1:11434/v1`（或自定义 base URL）。

### 答案质量差（答非所问）
资料未命中时模型会基于自身知识回答（v0.1.0 策略：资料为辅助参考）。若仍出现「资料中未涉及」式回答，确认 `answer/prompt.rs` 规则 2/5 已生效（重新构建后生效）。

## 数据与凭据

### 清除数据
设置页「清除全部数据」会删除会议/转写/问题/答案/资料（保留设置与 API Key）。

### 凭据管理器不可用
API Key 无法保存时，连接测试会失败。检查 Windows 凭据管理器服务（Credential Manager）是否运行。

## 其他

### 首次导入模型后仍需「扫描并校验」
模型导入登记在 `models/registry.json`；手动拷贝文件不会自动登记，需在设置页扫描。

### 置顶小窗口无内容
置顶窗依赖会话事件；开始会话并产生问题/答案后才会显示内容（静态待机时不显示）。
