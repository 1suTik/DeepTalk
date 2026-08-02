#requires -Version 5.1
<#
.SYNOPSIS
在本会话中配置 Rust/Tauri 构建所需环境变量（VS 开发者环境 + Vulkan/LLVM/CMake/Ninja + 短路径 target 目录）。

重要：必须在当前会话中运行，不要使用 `powershell -File`（那会在子进程里设置变量，退出后失效）。

.EXAMPLE
# 方式一（推荐，需先放开执行策略一次）：
#   Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
#   .\scripts\build-env.ps1
#   cargo test --manifest-path src-tauri/Cargo.toml audio::
#
# 方式二（不改执行策略，一次性）：
#   powershell -ExecutionPolicy Bypass -Command ".\scripts\build-env.ps1; cargo test --manifest-path src-tauri/Cargo.toml audio::"
#>
$ErrorActionPreference = "Stop"

$vcvars = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
if (Test-Path -LiteralPath $vcvars) {
    cmd /c "call `"$vcvars`" && set" | ForEach-Object {
        if ($_ -match "^(.*?)=(.*)$") {
            Set-Item -Path "Env:$($matches[1])" -Value $matches[2]
        }
    }
} else {
    Write-Warning "vcvars64.bat not found; VS developer environment not loaded."
}

# 工具链统一目录（2026-08-03 从 C 盘迁移）
$buildTools = "G:\Project\build-tools"

$vulkanRoot = Join-Path $buildTools "VulkanSDK"
$vulkanSdk = Get-ChildItem $vulkanRoot -Directory -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending | Select-Object -First 1
if ($vulkanSdk) {
    Set-Item -Path "Env:VULKAN_SDK" -Value $vulkanSdk.FullName
    $env:Path = "$($vulkanSdk.FullName)\Bin;$env:Path"
} else {
    Write-Warning "Vulkan SDK not found under $vulkanRoot"
}

$llvm = Join-Path $buildTools "LLVM\bin"
if (Test-Path -LiteralPath "$llvm\libclang.dll") {
    Set-Item -Path "Env:LIBCLANG_PATH" -Value $llvm
    $env:Path = "$llvm;$env:Path"
} else {
    Write-Warning "LLVM not found at $llvm"
}

$cmake = Join-Path $buildTools "CMake\bin"
if (Test-Path -LiteralPath "$cmake\cmake.exe") {
    $env:Path = "$cmake;$env:Path"
} else {
    Write-Warning "CMake not found at $cmake"
}

$ninja = Join-Path $buildTools "ninja"
if (Test-Path -LiteralPath "$ninja\ninja.exe") {
    $env:Path = "$ninja;$env:Path"
} else {
    Write-Warning "Ninja not found at $ninja"
}

Set-Item -Path "Env:CMAKE_GENERATOR" -Value "Ninja"
Set-Item -Path "Env:CARGO_TARGET_DIR" -Value "G:\t"

# Rust 工具链：未持久化时自愈（正常由 setx 提供）
if (-not $env:RUSTUP_HOME) {
    Set-Item -Path "Env:RUSTUP_HOME" -Value (Join-Path $buildTools "rust\.rustup")
}
if (-not $env:CARGO_HOME) {
    Set-Item -Path "Env:CARGO_HOME" -Value (Join-Path $buildTools "rust\.cargo")
}
$cargoBin = Join-Path $env:CARGO_HOME "bin"
if (Test-Path -LiteralPath "$cargoBin\cargo.exe") {
    $env:Path = "$cargoBin;$env:Path"
} else {
    Write-Warning "cargo not found at $cargoBin"
}

Write-Host "Build environment ready:" -ForegroundColor Green
Write-Host "  RUSTUP_HOME      = $env:RUSTUP_HOME"
Write-Host "  CARGO_HOME       = $env:CARGO_HOME"
Write-Host "  VULKAN_SDK       = $env:VULKAN_SDK"
Write-Host "  LIBCLANG_PATH    = $env:LIBCLANG_PATH"
Write-Host "  CMAKE_GENERATOR  = $env:CMAKE_GENERATOR"
Write-Host "  CARGO_TARGET_DIR = $env:CARGO_TARGET_DIR"
Write-Host "Now run: npm run tauri dev"
