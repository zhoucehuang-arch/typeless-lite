# Typeless Lite

轻量化语音输入工具重建版。当前实现按 `docs/typeless-rebuild-plan.md` 推进，目标是保留最核心的五项能力：语音输入、自动润色、个性化设置、词典、历史记录。

## 当前状态

- `app/`：Tauri 2 + Rust + React/TypeScript 应用骨架。
- 前端页面：首页、历史、词典、风格、设置。
- 后端模块：录音、Windows 本地 sherpa-onnx ASR、OpenAI-compatible 润色、本地 JSON 持久化、系统凭据库、剪贴板插入兜底、全局热键。
- 规划文档：`docs/typeless-rebuild-plan.md`。
- Windows 发布：`.github/workflows/release-windows.yml` 会在推送 `v*` tag 时构建并上传 NSIS `.exe` 安装包。
- 代码签名：workflow 已预留 PFX 证书签名入口，配置方式见 `docs/release-windows.md`。

## 开发

```bash
cd app
npm install
npm run build
npm run tauri dev
```

Rust 工具链是运行 Tauri/Rust 检查的前提：

```bash
cargo check --manifest-path app/src-tauri/Cargo.toml
```

## Windows 打包发布

自动发布到 GitHub Release：

```bash
git tag v0.1.1
git push origin v0.1.1
```

也可以在 Windows 本地构建安装包：

```powershell
cd app
powershell -ExecutionPolicy Bypass -File .\scripts\windows-build.ps1
```

详细说明见 `docs/release-windows.md`。

## 本地 ASR 模型

首版只接本地 `sherpa-onnx`。模型文件不打包进仓库，需要放到应用数据目录下的模型子目录。

Windows 默认目录：

```text
%APPDATA%\TypelessLite\models\sherpa-onnx\<model-alias>\
```

默认模型 alias：

```text
sense-voice-small-zh
```

该目录至少需要：

```text
model.int8.onnx
tokens.txt
```
