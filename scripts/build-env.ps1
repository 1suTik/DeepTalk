#requires -Version 5.1
<#
.SYNOPSIS
在本会话中配置 Rust/Tauri 构建所需环境变量（VS 开发者环境 + Vulkan/LLVM/CMake/Ninja + 短路径 target 目录）。
在 VSCode PowerShell 中执行本脚本后，可直接运行 `npm run tauri dev`。

.EXAMPLE
powershell -ExecutionPolicy Bypass -File scripts/build-env.ps1
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

$vulkanSdk = Get-ChildItem "C:\VulkanSDK" -Directory -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending | Select-Object -First 1
if ($vulkanSdk) {
    Set-Item -Path "Env:VULKAN_SDK" -Value $vulkanSdk.FullName
    $env:Path = "$($vulkanSdk.FullName)\Bin;$env:Path"
}

$llvm = "C:\Program Files\LLVM\bin"
if (Test-Path -LiteralPath "$llvm\libclang.dll") {
    Set-Item -Path "Env:LIBCLANG_PATH" -Value $llvm
    $env:Path = "$llvm;$env:Path"
}

$cmake = "C:\Program Files\CMake\bin"
if (Test-Path -LiteralPath "$cmake\cmake.exe") {
    $env:Path = "$cmake;$env:Path"
}

$ninja = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Recurse -Filter ninja.exe -ErrorAction SilentlyContinue |
    Select-Object -First 1 -ExpandProperty FullName
if ($ninja) {
    $env:Path = "$(Split-Path -Parent $ninja);$env:Path"
}

Set-Item -Path "Env:CMAKE_GENERATOR" -Value "Ninja"
if (-not $env:CARGO_TARGET_DIR) {
    Set-Item -Path "Env:CARGO_TARGET_DIR" -Value "G:\t"
}
Set-Item -Path "Env:CARGO_TARGET_DIR" -Value "G:\t"
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

Write-Host "Build environment ready:" -ForegroundColor Green
Write-Host "  VULKAN_SDK       = $env:VULKAN_SDK"
Write-Host "  LIBCLANG_PATH    = $env:LIBCLANG_PATH"
Write-Host "  CMAKE_GENERATOR  = $env:CMAKE_GENERATOR"
Write-Host "  CARGO_TARGET_DIR = $env:CARGO_TARGET_DIR"
Write-Host "Now run: npm run tauri dev"
