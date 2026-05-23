# Typeless Lite 测试流程

本流程是每次优化完成前的最低验收门槛。

## 本地检查

1. 前端类型和产物构建：

```bash
cd app
npm run build
```

2. 代码空白和补丁格式：

```bash
git diff --check
```

3. OpenAI-compatible 中转端点冒烟：

- Base URL `https://ai.input.im` 必须自动尝试 `/v1/models`。
- 模型列表能返回非空 JSON。
- Chat Completions 必须自动尝试 `/v1/chat/completions` 并返回内容。
- API Key 只能通过临时环境变量传入，不写入仓库、日志或配置文件。

4. 功能排除项：

```bash
rg -n "openless-ime|installerHooks|componentRefs|TSF|IME|输入法" app .github
```

除文档说明外，应用代码和打包配置不得包含 OpenLess 的 Windows 输入法植入逻辑。

## CI 检查

本机没有 Rust 工具链时，Rust/Tauri 编译以 GitHub Actions 为准：

1. 推送提交和版本 tag。
2. 等待 `.github/workflows/release-windows.yml` 通过。
3. 确认 GitHub Release 存在 `Typeless-Lite_<version>_Windows_x64_Setup.exe`。

## 手工回归

在 Windows 安装 Release EXE 后执行：

1. 点击主窗口关闭按钮，窗口隐藏，热键仍可触发听写。
2. 默认右 Alt 按住说话可开始和结束听写。
3. Base URL 填 `https://ai.input.im`，API Key 填入后可获取模型列表、选择模型并检验可用。
4. 听写完成后胶囊出现在屏幕底部，不依附在主窗口底部。
5. 新听写记录自动出现在历史记录，无需手动刷新。
