# Typeless Rebuild 产品结构与实施计划

> 工作名：Typeless Lite  
> 目标平台：Windows 优先，后续可自然扩展 macOS  
> 参考对象：OpenLess 开源实现、Typeless 公开产品形态  
> 当前文档目标：定义从零重建的产品边界、技术结构、数据模型和实施路线；不复刻 OpenLess 的复杂功能，不写业务实现。

## 1. 结论摘要

OpenLess 的核心链路值得保留：全局热键触发录音，本地 ASR 得到原始转写，LLM API 做润色，再把结果插入当前光标，失败时复制到剪贴板。它的技术栈也适合本项目：Tauri 2 + Rust 后端 + React/TypeScript 前端，Rust 负责录音、热键、ASR、插入和本地持久化，React 负责轻量控制面板。

但 OpenLess 已经明显超出本项目需要：它包含云端 ASR、多 provider 管理、翻译热键、划词问答面板、风格市场、多语言 UI、自动更新、Beta 渠道、Windows TSF/IME 插件、Linux fcitx 插件、录音调试归档等。本重建版应只保留五件事：

1. 语音输入
2. 自动润色，支持风格切换
3. 个性化设置
4. 词典功能
5. 历史记录

推荐做法不是 fork OpenLess 后删功能，而是从零建一个轻量 Tauri 项目，借鉴 OpenLess 的模块边界和关键实现思路，保留少量可验证的工程模式：统一 Coordinator、AudioConsumer 式 ASR、JSON 本地存储、系统凭据库、OpenAI-compatible LLM 客户端、剪贴板插入兜底。

## 2. 参考分析

### 2.1 OpenLess 可借鉴点

OpenLess 的主链路很清晰：

```text
global hotkey
  -> recorder captures 16 kHz mono PCM
  -> ASR provider transcribes
  -> active style prompt calls LLM
  -> post-process dictionary/correction
  -> insert into current app or copy fallback
  -> append local history
```

值得复用的设计：

- **Tauri 2 桌面壳**：安装包轻，Rust 能直接处理系统权限、热键、音频、剪贴板和输入模拟。
- **Rust Coordinator**：把 session phase 收敛到一个状态机，避免录音、转写、润色、插入互相抢状态。
- **录音接口**：`cpal` 采集，统一重采样为 16 kHz / mono / Int16 PCM，ASR provider 只实现消费 PCM 和转写。
- **本地持久化**：历史、词典、偏好、风格包用 JSON 文件；API key 放系统凭据库。
- **润色 Provider**：OpenAI-compatible Chat Completions 足够覆盖 OpenAI、DeepSeek、Ark、OpenRouter、自建代理等。
- **词典双注入**：词典同时进入 ASR hotwords / prompt hint，最终文本再统计命中次数。
- **失败语义**：润色失败插入原文，插入失败复制到剪贴板，尽量做到“用户说过的话不丢”。

### 2.2 OpenLess 应删除点

重建版明确不做这些：

- 云端 ASR provider 矩阵：Volcengine、Whisper-compatible、Bailian 等。
- 翻译模式和翻译热键。
- 划词语音问答面板。
- Style Pack Marketplace、导入导出、发布和点赞。
- 多语言 UI，首版只做简体中文界面。
- 自动更新、Beta 渠道、发布 manifest。
- Linux 支持和 fcitx5 插件。
- Windows TSF 输入法插件首版不做，先用剪贴板粘贴和 SendInput/Enigo 兜底。
- 录音调试归档、音频导出、复杂隐私开关。
- 对话感知 polish 历史上下文，首版每次听写独立处理。

### 2.3 Typeless 功能参考

Typeless 的公开定位是商业语音输入：跨桌面和移动端，把语音变成可直接使用的文本，并强调自动编辑、快捷输入、不同写作风格、个人词汇/偏好等体验。本重建版只参考它的轻量交互形态：

- 常驻后台，用户在任意输入框用快捷键说话。
- 小型浮窗显示录音、识别、润色、完成状态。
- 设置里能选默认风格、模型服务、快捷键、麦克风、个人词典。
- 历史里能找回每次原文和润色后文本。

不参考 Typeless 的订阅、账号、移动端同步、团队功能、模板动作或复杂编辑能力。

## 3. 产品范围

### 3.1 MVP 功能

**语音输入**

- 全局快捷键：默认 `Ctrl+Space`，支持自定义。
- 录音模式：默认按住说话，另支持点击切换。
- 本地 ASR：Windows 首选 `sherpa-onnx` 离线模型。
- 插入策略：优先粘贴到当前光标，失败时复制并提示用户手动粘贴。

**自动润色**

- 内置 4 种风格：原文、轻度润色、清晰结构、正式表达。
- 主界面可一键切换当前风格。
- 每个风格有固定系统提示词，后续允许用户编辑。
- 润色通过 OpenAI-compatible API 调用。

**个性化设置**

- 快捷键、录音模式、麦克风、启动项。
- ASR 模型选择、模型下载/删除、语言 hint。
- LLM base URL、API key、模型名、temperature。
- 输出语言偏好：自动、简体中文、英文。
- 剪贴板恢复开关、历史保留条数。

**词典功能**

- 用户词条：短语、备注、启用状态、命中次数。
- 纠错规则：`错误词 -> 正确词`。
- ASR 阶段尽量注入 hotwords；LLM 阶段注入词典提示。
- 最终输出再应用简单纠错规则。

**历史记录**

- 保存原始转写、最终文本、风格、时间、耗时、插入状态、错误码、词典命中数。
- 支持搜索、按风格筛选、复制、删除、清空。
- 默认保留最近 200 条，可配置。

### 3.2 非目标

- 不做聊天助手。
- 不做问答。
- 不做翻译。
- 不做账号和云同步。
- 不做插件市场。
- 不做移动端。
- 不做复杂模板动作。
- 不做多人/团队功能。

## 4. 技术方案

### 4.1 技术栈

```text
Desktop shell: Tauri 2
Backend:       Rust 2021
Frontend:      React 18 + TypeScript + Vite
Audio:         cpal
ASR:           sherpa-onnx local offline models
LLM:           OpenAI-compatible Chat Completions over reqwest
Storage:       JSON files + OS credential vault
Clipboard:     arboard
Input:         enigo / Windows SendInput fallback
```

首版只承诺 Windows。架构保留跨平台边界，但不为了 macOS/Linux 提前引入复杂适配。

### 4.2 推荐目录结构

```text
Typeless/
  docs/
    typeless-rebuild-plan.md
  app/
    package.json
    vite.config.ts
    src/
      App.tsx
      main.tsx
      components/
        Capsule.tsx
        Shell.tsx
        SettingsModal.tsx
      pages/
        Home.tsx
        History.tsx
        Dictionary.tsx
        Styles.tsx
      lib/
        ipc.ts
        types.ts
        hotkey.ts
      styles/
        global.css
    src-tauri/
      Cargo.toml
      tauri.conf.json
      src/
        main.rs
        lib.rs
        commands.rs
        coordinator.rs
        recorder.rs
        insertion.rs
        persistence.rs
        credentials.rs
        polish.rs
        hotkey.rs
        types.rs
        asr/
          mod.rs
          sherpa.rs
          models.rs
```

### 4.3 后端模块

**Coordinator**

唯一负责听写 session 状态：

```text
Idle
  -> Starting
  -> Listening
  -> Transcribing
  -> Polishing
  -> Inserting
  -> Idle
```

它拥有 recorder、active ASR、preferences store、history store、dictionary store 和 inserter。所有热键、取消、错误都通过 Coordinator 收敛。

**Recorder**

- 使用 `cpal` 打开输入设备。
- 输出 16 kHz / mono / Int16 PCM。
- 通过 level callback 更新浮窗音量。
- 通过 `AudioConsumer` trait 把 PCM 推给 ASR provider。

**ASR**

首版只实现本地 `sherpa-onnx`：

- 默认模型建议：SenseVoice small int8，中文/中英混合优先。
- 可选模型：Paraformer zh int8、Whisper small multilingual int8。
- 模型不打包，首次启用时下载到 `%APPDATA%/Typeless/models/sherpa-onnx/<alias>/`。
- MVP 先做 batch 转写：录音结束后统一推理。
- 后续再做 streaming partial transcript。

**Polish**

- 一个 OpenAI-compatible client。
- 请求字段：`base_url`、`api_key`、`model`、`temperature`、`messages`。
- 只输出最终文本，不输出解释。
- 如果 LLM 失败，返回原始转写并记录 `polishFailed`。

**Insertion**

- 保存目标焦点信息。
- 转写结束后恢复焦点。
- 写剪贴板并模拟 `Ctrl+V`。
- 可选恢复用户原剪贴板。
- 失败时只复制，不丢文本。

**Persistence**

数据目录：

```text
Windows: %APPDATA%/Typeless/
```

文件：

```text
preferences.json
history.json
dictionary.json
correction-rules.json
styles.json
models/
logs/
```

凭据：

- `llm.api_key` 放 Windows Credential Manager。
- 其他非敏感配置放 `preferences.json`。

### 4.4 前端信息架构

主窗口保持轻：

```text
Sidebar
  Home
  History
  Dictionary
  Styles
  Settings
```

**Home**

- 当前风格。
- 当前快捷键。
- 今日听写次数/字数。
- ASR 模型状态。
- LLM 配置状态。
- 最近 5 条历史。

**History**

- 左侧列表，右侧详情。
- 展示原文和最终文本。
- 复制、删除、清空。
- 按风格筛选。

**Dictionary**

- 添加/启用/禁用/删除词条。
- 显示命中次数。
- 纠错规则：错误词、替换词。

**Styles**

- 四个内置风格卡片。
- 当前风格一键激活。
- 简单编辑提示词。
- 重置内置风格。

**Settings**

- General：快捷键、录音模式、启动项、浮窗。
- Audio & ASR：麦克风、本地模型、语言 hint。
- LLM：base URL、API key、model、temperature、测试连接。
- Privacy & Data：历史保留、清空数据、剪贴板恢复。

**Capsule**

状态足够简单：

```text
idle / recording / transcribing / polishing / done / cancelled / error
```

浮窗只显示状态、音量、电平、耗时、错误短提示。

## 5. 数据模型草案

### 5.1 Preferences

```ts
type PolishMode = 'raw' | 'light' | 'structured' | 'formal';
type HotkeyMode = 'hold' | 'toggle';

interface Preferences {
  hotkey: string;
  hotkeyMode: HotkeyMode;
  launchAtLogin: boolean;
  showCapsule: boolean;
  microphoneDeviceName: string | null;

  activeStyleId: string;
  outputLanguage: 'auto' | 'zhCn' | 'en';

  asrProvider: 'sherpaOnnx';
  sherpaModel: string;
  sherpaLanguageHint: string | null;
  sherpaKeepLoadedSecs: number;

  llmBaseUrl: string;
  llmModel: string;
  llmTemperature: number;

  restoreClipboardAfterPaste: boolean;
  historyMaxEntries: number;
}
```

### 5.2 History

```ts
interface DictationSession {
  id: string;
  createdAt: string;
  rawTranscript: string;
  finalText: string;
  mode: PolishMode;
  insertStatus: 'inserted' | 'copiedFallback' | 'failed';
  errorCode: string | null;
  durationMs: number;
  dictionaryHitCount: number;
}
```

### 5.3 Dictionary

```ts
interface DictionaryEntry {
  id: string;
  phrase: string;
  note: string | null;
  enabled: boolean;
  hits: number;
  createdAt: string;
}

interface CorrectionRule {
  id: string;
  pattern: string;
  replacement: string;
  enabled: boolean;
  createdAt: string;
}
```

### 5.4 Style

```ts
interface StyleProfile {
  id: string;
  name: string;
  mode: PolishMode;
  prompt: string;
  builtin: boolean;
  updatedAt: string;
}
```

## 6. 默认润色风格

**原文**

只做标点、断句、明显错字修正，不改变语气和结构。

**轻度润色**

保留原意和表达习惯，去掉口癖，修正标点，让文本自然可发送。

**清晰结构**

把长口述整理为分段、列表、约束、待办，适合写需求、prompt、工作说明。

**正式表达**

整理为工作沟通或邮件风格，语气更稳，更少口语，更清楚地陈述事实和请求。

提示词约束必须包括：

- 不回答用户口述中的问题。
- 不执行请求。
- 不添加用户没有说过的事实。
- 不输出解释、前后缀、Markdown 围栏。
- 专有名词、代码、URL、数字尽量保留。
- 词典词条优先按用户给定写法输出。

## 7. 实施计划

### Phase 0：项目骨架

- 创建 Tauri 2 + React/TS + Rust 工作区。
- 配好基础窗口、日志、IPC、Windows 安装配置。
- 建立 `types.rs` 和 `lib/types.ts` 的类型镜像。

验收：

- `npm run build` 通过。
- `cargo check` 通过。
- 主窗口能打开，IPC 能返回版本信息。

### Phase 1：核心听写闭环

- 全局热键。
- `cpal` 录音。
- `sherpa-onnx` batch 转写。
- OpenAI-compatible 润色。
- 剪贴板插入兜底。
- Capsule 状态。

验收：

- 在记事本/浏览器输入框按快捷键说话，松开后文本落到光标处。
- LLM 失败时插入原文。
- 插入失败时文本留在剪贴板。

### Phase 2：本地数据

- `preferences.json`。
- `history.json`。
- `dictionary.json` 和 `correction-rules.json`。
- 系统凭据库存 API key。

验收：

- 重启后设置、词典、历史都保留。
- 删除/清空历史可靠。
- API key 不出现在明文 JSON。

### Phase 3：轻量 UI

- Home、History、Dictionary、Styles、Settings。
- Capsule 独立窗口。
- 设置表单和状态同步。

验收：

- 所有可见控件都有真实后端功能。
- 窗口缩放不遮挡核心控件。
- 无占位市场、账号、翻译、问答入口。

### Phase 4：稳定性和发布

- 热键冲突提示。
- 麦克风权限检测。
- 模型下载失败重试。
- LLM 连接测试。
- Windows 安装包。

验收：

- 冷启动、连续 20 次听写、取消、空录音、断网、模型缺失都有可理解状态。
- 基础 smoke 脚本覆盖核心命令。

## 8. 风险与取舍

**Windows 文本插入**

剪贴板粘贴在绝大多数场景够用，但某些管理员窗口、密码框、游戏、远程桌面可能失败。首版不做 TSF 输入法插件，避免把项目复杂度拉到 OpenLess 级别。

**本地 ASR 模型体积**

离线模型会带来下载和磁盘占用。首版不要内置大模型，只做模型管理页和清楚的下载状态。

**ASR 质量**

SenseVoice/Paraformer 对中文友好，但英文和中英混合场景需要实测。需要保留语言 hint，并允许用户切换 Whisper multilingual。

**LLM API 兼容**

OpenAI-compatible 并不代表所有供应商字段完全一致。首版只发最小请求字段，不做 thinking、tools、response_format 等扩展。

**隐私**

ASR 本地化能避免上传音频，但润色文本会发给用户配置的 LLM API。设置页必须明确显示这一点。

## 9. OpenLess 到重建版的映射

| OpenLess 模块 | 重建版处理 |
| --- | --- |
| Tauri + Rust + React | 保留 |
| Coordinator session state | 保留并简化 |
| cpal recorder | 保留 |
| Volcengine/Bailian/Whisper cloud ASR | 删除 |
| Qwen3 macOS local ASR | 暂不做 |
| Foundry Local Whisper | 暂不做 |
| sherpa-onnx Windows local ASR | 首版主 ASR |
| OpenAI-compatible LLM | 保留 |
| Gemini/Codex/Anthropic 特殊 provider | 删除 |
| Style Pack Marketplace | 删除 |
| 四种内置风格 | 保留 |
| 风格导入/导出/发布 | 删除 |
| 词典 hotwords + prompt hints | 保留 |
| correction rules | 保留简化 |
| History | 保留简化 |
| Translation | 删除 |
| Selection Ask QA | 删除 |
| Auto update/Beta | 删除 |
| 多语言 UI | 删除，首版中文 |
| Windows IME TSF 插件 | 暂不做 |

## 10. 第一版验收标准

第一版做到以下程度即可认为产品成立：

- 用户安装后配置本地 ASR 模型和 LLM API key。
- 在任意常见文本框按快捷键说话。
- 5 秒内完成短句转写、润色和插入。
- 能在四种风格之间切换。
- 词典能改善专有名词输出。
- 历史能找回最近输入。
- 断网、模型缺失、API 失败、插入失败都不丢文本。

核心原则：不要追 OpenLess 的全功能版，也不要追 Typeless 的商业完整度。这个重建版只做“轻、稳、可控”的语音输入工具。
