# Windows Release 打包说明

本项目已经配置 GitHub Actions，可以在 GitHub Release 中自动上传 Windows 安装包。

## 自动发布

推送版本 tag：

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions 会运行 `.github/workflows/release-windows.yml`，在 `windows-latest` 上执行 Tauri build，并把 NSIS 安装包上传到对应 GitHub Release。

Release 资产命名格式：

```text
Typeless-Lite_<version>_Windows_x64_Setup.exe
```

其中 `<version>` 来自 `app/src-tauri/tauri.conf.json` 里的 `version` 字段。

## 手动触发

也可以在 GitHub 仓库页面手动运行：

```text
Actions -> Release Windows -> Run workflow
```

手动触发会创建 prerelease，tag 形如：

```text
manual-<run_number>
```

## 本地 Windows 打包

在 Windows + Rust 工具链环境下：

```powershell
cd app
powershell -ExecutionPolicy Bypass -File .\scripts\windows-build.ps1
```

产物目录：

```text
app\src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\
```

本地脚本不会上传 Release，只会在上面的目录生成 NSIS `.exe` 安装包。

## 打包内容

- 打包目标：Windows x64 NSIS 安装包。
- 安装模式：当前用户安装，不默认要求管理员权限。
- 应用图标：`app/src-tauri/icons/icon.ico`。
- WebView2：安装器使用 Microsoft WebView2 download bootstrapper，安装包更小，但首次安装时可能需要联网补齐 WebView2 Runtime。

## 代码签名

workflow 已预留代码签名接入口。没有配置证书时，构建仍会成功，但产物是未签名 `.exe`，Windows SmartScreen 可能提示风险。

### 你需要准备什么

Windows 代码签名证书必须由你本人或你的组织购买并完成身份验证。常见选择：

- OV Code Signing Certificate：适合个人或公司发布，价格较低，但新证书初期仍可能有 SmartScreen 信誉积累期。
- EV Code Signing Certificate：通常需要硬件令牌或云签名，价格更高，SmartScreen 信誉更好。
- Microsoft Trusted Signing：适合公司/组织通过 Azure 做云签名，需要 Azure 账户和身份验证。

首版最简单的是购买可导出 `.pfx` 的 OV 代码签名证书。

### 配置 GitHub Secrets

拿到 `.pfx` 证书后，在 Windows PowerShell 里转成 base64：

```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("C:\path\to\certificate.pfx")) | Set-Content -NoNewline certificate.pfx.base64
```

在 GitHub 仓库中添加 Secrets：

```text
Settings -> Secrets and variables -> Actions -> New repository secret
```

需要添加：

```text
WINDOWS_CERTIFICATE_BASE64=<certificate.pfx.base64 的内容>
WINDOWS_CERTIFICATE_PASSWORD=<导出 pfx 时设置的密码>
```

可选添加 repository variable：

```text
WINDOWS_SIGNING_TIMESTAMP_URL=http://timestamp.digicert.com
```

### 签名实现位置

- Tauri 配置：`app/src-tauri/tauri.conf.json`
- 签名脚本：`app/src-tauri/scripts/windows-sign.ps1`
- Release workflow：`.github/workflows/release-windows.yml`

workflow 会把 `WINDOWS_CERTIFICATE_BASE64` 写成临时 `.pfx` 文件，Tauri 打包时调用 `signtool.exe` 对 Windows 产物签名。
