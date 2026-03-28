# FairyAction

AI 驱动的浏览器自动化 Agent，使用 Rust 构建。通过自然语言描述任务，AI 自动控制浏览器完成操作。

## 特性

- 🤖 **AI Agent** — 基于 LLM 的感知-决策-行动循环，自动完成复杂浏览器任务
- 🖥️ **TUI 测试器** — 交互式终端界面，实时查看 DOM 树、操控浏览器
- 🌐 **CDP 控制** — 通过 Chrome DevTools Protocol 精确控制 Chromium 浏览器
- 📐 **智能 DOM 提取** — 扁平化 DOM 表示，自动识别可交互元素，支持 `cursor: pointer` 检测
- 🔄 **多标签页管理** — 自动检测新标签页、切换 CDP 连接
- 🎯 **真实鼠标事件** — 模拟真实用户点击，支持 Vue/React 等前端框架
- 🔌 **OpenAI 兼容** — 支持所有兼容 OpenAI API 的 LLM 服务

## 快速开始

### 安装依赖

- [Rust](https://rustup.rs/) 1.85+
- [Chrome/Chromium](https://www.google.com/chrome/) 浏览器

### 编译

```bash
git clone https://github.com/your-username/FairyAction.git
cd FairyAction
cargo build --release
```

### 配置

创建 `.env` 文件或在 `~/.config/fairy-action/config.json` 中配置：

```bash
# 浏览器配置
FA_BROWSER_CHROME_PATH=C:\Program Files\Google\Chrome\Application\chrome.exe

# LLM 配置
FA_LLM_API_KEY=sk-your-api-key
FA_LLM_MODEL=gpt-4o
FA_LLM_BASE_URL=https://api.openai.com/v1
```

也可以通过 CLI 管理：

```bash
fairy-action config init                    # 初始化配置文件
fairy-action config show                    # 查看当前配置
fairy-action config set llm.api-key sk-xxx  # 设置 API Key
```

### 运行 AI Agent

```bash
# 自动模式 — AI 完成任务
fairy-action run "打开百度搜索 Rust 编程语言" --show-browser

# 指定模型和步数
fairy-action run "在 GitHub 上搜索 Rust 项目" \
  --model gpt-4o \
  --max-steps 50 \
  --show-browser

# 启用视觉模式（截图）
fairy-action run "查看今日新闻" --vision --show-browser

# 保存执行追踪
fairy-action run "爬取数据" --trace trace.jsonl --show-browser
```

### 启动 TUI 测试器

```bash
fairy-action tester
# 或
cargo run --bin fa-tester
```

## CLI 命令

```
fairy-action [OPTIONS] <COMMAND>

Options:
  -c, --config <FILE>     指定配置文件路径
  -v, --verbose           启用详细日志
  -h, --help              显示帮助
  -V, --version           显示版本

Commands:
  run       运行 AI Agent 执行任务
  tester    启动交互式 TUI 测试器
  config    配置管理
```

### `run` 子命令

```
fairy-action run <TASK> [OPTIONS]

参数:
  <TASK>                    任务描述

Options:
  -s, --max-steps <N>       最大步数 (默认 100)
  --vision                  启用视觉 (截图)
  -t, --trace <FILE>        保存执行追踪
  --show-browser            显示浏览器窗口
  --provider <PROVIDER>     LLM 提供商
  --model <MODEL>           LLM 模型
  --api-key <KEY>           API Key
  --base-url <URL>          LLM Base URL
```

### `config` 子命令

```
fairy-action config <ACTION>

Actions:
  show                      显示当前配置
  init                      初始化配置文件
  set <KEY> <VALUE>         设置配置项
```

## TUI 测试器命令

启动后进入交互式终端界面，支持以下命令：

### 导航

| 命令 | 用法 | 说明 |
|---|---|---|
| `navigate` | `navigate https://example.com` | 导航到 URL |
| `back` | `back` | 浏览器后退 |
| `forward` | `forward` | 浏览器前进 |
| `reload` | `reload` | 刷新页面 |

### 交互

| 命令 | 用法 | 说明 |
|---|---|---|
| `click` | `click 5` | 点击指定索引的元素 |
| `input` | `input 3 hello world` | 在元素中输入文本 |
| `scroll` | `scroll down 500` | 滚动页面（up/down + 像素） |
| `press` | `press Enter` | 发送按键 |
| `eval` | `eval document.title` | 执行 JavaScript |

### 标签页

| 命令 | 用法 | 说明 |
|---|---|---|
| `tab-new` | `tab-new https://example.com` | 打开新标签页 |
| `tab-switch` | `tab-switch 0` | 切换到指定标签页 |
| `tab-close` | `tab-close 1` | 关闭指定标签页 |

### 其他

| 命令 | 别名 | 用法 | 说明 |
|---|---|---|---|
| `screenshot` | `ss` | `screenshot` | 截取屏幕截图 |
| `dom` | — | `dom` | 刷新 DOM 树 |
| `find` | — | `find 关键词` | 在页面中搜索文本 |
| `url` | — | `url` | 显示当前 URL |
| `title` | — | `title` | 显示页面标题 |
| `clear` | — | `clear` | 清除日志 |
| `help` | `?` | `help` | 切换帮助面板 |
| `quit` | `exit` | `quit` | 退出 |

### 快捷键

| 快捷键 | 功能 |
|---|---|
| `F5` | 刷新 DOM 树 |
| `Ctrl+R` | 刷新页面 |
| `Ctrl+D` | 截图 |
| `Ctrl+N` | 新标签页 |
| `Ctrl+W` | 关闭标签页 |
| `Ctrl+C` | 退出 |
| `PageUp / PageDown` | 滚动 DOM 树视图 |
| `Shift + ↑/↓` | DOM 树逐行滚动 |
| `Shift + Home/End` | DOM 树跳到顶部/底部 |
| `Esc` | 清空命令输入 |

### TUI 界面布局

```
┌──────────────────────────────────────────────────────────────────┐
│  FairyAction Tester  https://example.com | Page Title            │
├────────────────────────────────────────┬─────────────────────────┤
│                                        │  Status                 │
│  DOM Tree (F5 refresh, PgUp/PgDn)     │  URL: ...               │
│                                        │  Title: ...             │
│  [0] <a href="/home"> "Home"          │  Tab: 1/3               │
│  [1] <button> "Submit"                │                         │
│  [2] input placeholder="Search"       ├─────────────────────────┤
│  [3] <a href="/about"> "About"        │  Log                    │
│  ...                                   │  > Clicked element [1]  │
│                                        │  > Navigated to: ...    │
│                                        │  > DOM refreshed: 42    │
│                                        │  > interactive elements │
├────────────────────────────────────────┴─────────────────────────┤
│  Command (type 'help' for commands) > _                           │
└──────────────────────────────────────────────────────────────────┘
```

## 配置参考

所有配置可通过环境变量或配置文件设置。

### 浏览器配置

| 环境变量 | 默认值 | 说明 |
|---|---|---|
| `FA_BROWSER_HEADLESS` | `true` | 无头模式 |
| `FA_BROWSER_VIEWPORT_WIDTH` | `1280` | 视口宽度 |
| `FA_BROWSER_VIEWPORT_HEIGHT` | `720` | 视口高度 |
| `FA_BROWSER_CHROME_PATH` | 自动检测 | Chrome 可执行文件路径 |
| `FA_BROWSER_PROXY` | — | 代理地址 |

### LLM 配置

| 环境变量 | 默认值 | 说明 |
|---|---|---|
| `FA_LLM_PROVIDER` | `openai` | LLM 提供商 |
| `FA_LLM_MODEL` | `gpt-4o` | 模型名称 |
| `FA_LLM_API_KEY` | — | API Key |
| `FA_LLM_BASE_URL` | — | API Base URL（兼容 OpenAI 格式即可） |
| `FA_LLM_MAX_TOKENS` | `4096` | 最大输出 token |
| `FA_LLM_TEMPERATURE` | `0.0` | 温度 |

### Agent 配置

| 环境变量 | 默认值 | 说明 |
|---|---|---|
| `FA_AGENT_MAX_STEPS` | `100` | 最大执行步数 |
| `FA_AGENT_MAX_FAILURES` | `5` | 最大连续失败次数 |
| `FA_AGENT_USE_VISION` | `true` | 启用视觉（截图） |

## Agent 工作原理

Agent 采用经典的 **感知 → 决策 → 行动** 循环：

```
┌─────────────────────────────────────┐
│  每一步:                             │
│                                     │
│  1. 感知 (Perceive)                  │
│     └─ 获取页面 DOM 状态 + 截图      │
│                                     │
│  2. 决策 (Decide)                    │
│     └─ 调用 LLM 分析状态并输出动作   │
│                                     │
│  3. 行动 (Act)                       │
│     └─ 执行 LLM 返回的浏览器操作     │
│                                     │
│  4. 循环检测                         │
│     └─ 检测重复状态避免死循环         │
│                                     │
│  5. 判断完成或继续                    │
└─────────────────────────────────────┘
```

### 支持的 Agent 动作（共 20 个）

**导航类**: `navigate` `go_back` `go_forward` `reload` `search`

**交互类**: `click` `input` `scroll` `send_keys` `select_option`

**页面类**: `screenshot` `extract` `switch_tab` `close_tab` `new_tab` `evaluate`

**文件类**: `save_to_file` `read_file`

**元动作**: `wait` `done`

## 项目结构

```
FairyAction/
├── crates/
│   ├── fa-cli/        # CLI 入口 (clap)
│   ├── fa-tester/     # TUI 测试器 (ratatui + crossterm)
│   ├── fa-agent/      # AI Agent 核心循环
│   ├── fa-browser/    # 浏览器控制 (CDP WebSocket)
│   ├── fa-dom/        # DOM 解析与序列化
│   ├── fa-actor/      # 页面元素操作
│   ├── fa-llm/        # LLM 集成 (OpenAI 兼容)
│   ├── fa-tools/      # 工具注册与执行引擎
│   └── fa-config/     # 配置管理
├── .env               # 环境变量配置
└── Cargo.toml         # Workspace 根配置
```

## 技术栈

| 组件 | 技术 |
|---|---|
| 语言 | Rust (Edition 2024) |
| 异步运行时 | Tokio |
| 浏览器控制 | CDP (Chrome DevTools Protocol) via tokio-tungstenite |
| TUI 框架 | ratatui + crossterm |
| HTTP 客户端 | reqwest (rustls) |
| CLI 解析 | clap |
| 序列化 | serde + serde_json |
| 日志 | tracing + tracing-subscriber |

## License

MIT License. Copyright (c) 2026 FairyAction.
