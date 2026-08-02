# 构建文档

## 环境要求

| 组件 | 版本 |
|---|---|
| Node.js | 22+（本机 24.18.1 验证） |
| Rust | stable MSVC（rustup 1.97.1 验证） |
| Visual Studio | 2022 Community + C++ 桌面开发（MSVC v143，含 cl.exe） |
| Windows SDK | 10.0.26100 |
| CMake | 4.4+（Vulkan/whisper.cpp 构建需要） |
| Ninja | 1.12+ |
| Vulkan SDK | 1.4.350.0（whisper.cpp Vulkan 后端编译需要） |
| LLVM | （onnxruntime 绑定需要 `LIBCLANG_PATH`） |

## 环境变量（本机约定）

```powershell
$env:CARGO_TARGET_DIR = "G:\t"          # 260 字符路径规避；勿改回 src-tauri\target
$env:RUSTUP_HOME = "G:\Project\build-tools\rust\.rustup"
$env:CARGO_HOME = "G:\Project\build-tools\rust\.cargo"
$env:VULKAN_SDK = "G:\Project\build-tools\VulkanSDK\1.4.350.0"
$env:LIBCLANG_PATH = "G:\Project\build-tools\LLVM\bin"
$env:CMAKE_GENERATOR = "Ninja"
```

工具链统一位于 `G:\Project\build-tools\`（rust\.cargo、rust\.rustup、VulkanSDK、LLVM、CMake、ninja）；CMake/Ninja/cargo 通过 PATH 解析，Vulkan SDK 的 Bin 与 CMake\bin 已同步替换系统 PATH 条目。

每次构建在 **vcvars64 环境**下执行（`cmd /c "call vcvars64.bat && cargo ..."`）；新终端先运行 `.\scripts\build-env.ps1`（须当前会话执行）。

## 构建步骤

```powershell
npm install                    # 前端依赖（package-lock.json 锁定）
cargo build --manifest-path src-tauri/Cargo.toml
npm run build                  # 前端产物到 dist/
npm run tauri dev              # 开发运行
npm run tauri build            # 生产构建 + NSIS 安装包
```

## 测试

```powershell
npm test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored asr::whisper_worker   # 需要本地已导入模型
```

## 依赖复现

- 前端：`package-lock.json`
- Rust：`src-tauri/Cargo.lock`
- 模型清单：`src-tauri/models/models.json`（元数据与 SHA-256，模型文件不提交 Git）

## 已知构建注意

- cl.exe 不支持超长路径：必须使用 `CARGO_TARGET_DIR` 到短路径（如 `G:\t`）。
- whisper-rs `vulkan` feature 需要 Vulkan SDK 头文件；多显卡机器构建不受影响（运行时才枚举设备）。
- ort 2.0-rc 使用 `download-binaries` 自动获取 ONNX Runtime 预编译二进制（首次构建联网）。
