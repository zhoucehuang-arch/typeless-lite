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
rg -n "openless-ime|installerHooks|componentRefs|TSF|IME|输入法" app/src app/src-tauri .github docs -S \
  -g '!app/node_modules/**' -g '!app/src-tauri/target/**' -g '!app/dist/**'
```

除文档说明外，应用代码和打包配置不得包含 OpenLess 的 Windows 输入法植入逻辑。

5. 敏感信息扫描：

```bash
rg -n "g""hp_|s""k-[A-Za-z0-9]" . -S \
  -g '!app/node_modules/**' -g '!app/src-tauri/target/**' -g '!app/dist/**'
```

不得把 GitHub Token、LLM API Key 或用户本地配置写入仓库。

## CI 检查

本机没有 Rust 工具链时，Rust/Tauri 编译以 GitHub Actions 为准：

1. 推送提交和版本 tag。
2. 等待 `.github/workflows/release-windows.yml` 通过。
3. 确认 GitHub Release 存在 `Typeless-Lite_<version>_Windows_x64_Setup.exe`。
4. 安装包启动后不能出现额外命令行窗口。
5. 应用图标、安装器图标、任务栏图标和托盘图标必须统一。

## 手工回归

在 Windows 安装 Release EXE 后执行：

1. 点击主窗口关闭按钮，窗口隐藏，热键仍可触发听写。
2. 点击关闭后，系统托盘三角区能看到 Typeless Lite 图标；左键托盘图标或菜单“显示 Typeless Lite”可恢复主窗口，菜单“退出 Typeless Lite”才真正退出。
3. 默认右 Alt 按住说话可开始和结束听写。
4. 进入录音状态后，悬浮胶囊出现取消按钮、对勾结束按钮和多条波形；点击对勾应结束录音并进入识别/润色。
5. Base URL 填 `https://ai.input.im`，API Key 填入后可获取模型列表、选择模型并检验可用。
6. 听写完成后胶囊出现在屏幕底部，不依附在主窗口底部。
7. 新听写记录自动出现在历史记录，无需手动刷新。
8. 润色回归：轻度润色不得扩写或输出“我整理如下”等前缀；清晰结构在 2 条以上事项时必须编号；正式表达不得凭空添加事实、署名、日期或承诺；词典热词应优先纠正 ASR 的同音、近音或音译错误。
