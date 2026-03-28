# FairyAction

AI Agent 的浏览器自动化基础设施，使用 Rust 构建。提供精细的浏览器控制能力，通过 JSON 协议暴露所有动作接口，供外部 AI Agent 调用。

## 特性

- **JSON 协议接口** — 通过 stdin/stdout 交互，外部 Agent 发送 JSON 指令即可控制浏览器
- **20 个内置动作** — 导航、点击、输入、滚动、标签页管理、文件操作等完整覆盖
- **动作自省** — 支持动态查询所有可用动作及 JSON Schema，方便 Agent 自适应集成
- **CDP 控制** — 通过 Chrome DevTools Protocol 精确控制 Chromium 浏览器
- **智能 DOM 提取** — 扁平化 DOM 表示，自动识别可交互元素，支持 `cursor: pointer` 检测
- **真实鼠标事件** — 模拟真实用户点击，支持 Vue/React 等前端框架
- **多标签页管理** — 自动检测新标签页、切换 CDP 连接
- **TUI 测试器** — 交互式终端界面，实时查看 DOM 树、手动操控浏览器

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
FA_BROWSER_CHROME_PATH=C:\Program Files\Google\Chrome\Application\chrome.exe
FA_BROWSER_HEADLESS=true
```

也可以通过 CLI 管理：

```bash
fairy-action config init                          # 初始化配置文件
fairy-action config show                          # 查看当前配置
fairy-action config set browser.headless false    # 显示浏览器窗口
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
  run              启动浏览器，通过 stdin/stdout JSON 协议接收动作指令
  list-actions     列出所有可用动作及其参数定义
  tester           启动交互式 TUI 测试器
  config           配置管理
```

## JSON 协议接口

### 启动

```bash
fairy-action run
fairy-action run --show-browser    # 显示浏览器窗口
```

启动后，FairyAction 通过 **stdin** 接收 JSON 请求，通过 **stdout** 返回 JSON 响应。

### 请求格式

```jsonc
// 执行动作
{"type": "execute", "action": "navigate", "params": {"url": "https://example.com"}}

// 获取浏览器状态
{"type": "get_state"}

// 获取当前页面 DOM
{"type": "get_dom"}

// 列出可用动作
{"type": "list_actions"}

// 关闭浏览器
{"type": "close"}
```

### 响应格式

```jsonc
// 动作执行结果
{"type": "ok", "action": "navigate", "result": {"success": true, "output": "Navigated to https://example.com"}}

// 动作执行失败
{"type": "ok", "action": "click", "result": {"success": false, "output": null, "error": "Element with index 99 not found"}}

// 浏览器状态
{"type": "state", "url": "https://example.com", "title": "Example", "tabs": [{"id": "...", "url": "...", "title": "...", "is_active": true}]}

// DOM 内容
{"type": "dom", "representation": "[0] <button> \"Submit\"\n[1] input placeholder=\"Search\"\n...", "element_count": 42}

// 动作列表（含 JSON Schema）
{"type": "actions", "actions": [...], "schema": {...}}

// 错误
{"type": "error", "message": "Unknown action: 'xxx'"}

// 关闭确认
{"type": "closed"}
```

### 集成示例

外部 AI Agent 的典型集成流程：

```
1. 启动 fairy-action run 进程
2. 发送 {"type": "list_actions"} 获取可用动作
3. 发送 {"type": "get_dom"} 获取页面 DOM
4. 根据任务需求，发送 {"type": "execute", "action": "click", "params": {"index": 5}} 执行操作
5. 重复 3-4 直到任务完成
6. 发送 {"type": "close"} 关闭浏览器
```

## 可用动作（共 20 个）

### 导航类

| 动作 | 参数 | 说明 |
|---|---|---|
| `navigate` | `url` (必填), `new_tab` (可选) | 导航到 URL |
| `go_back` | — | 浏览器后退 |
| `go_forward` | — | 浏览器前进 |
| `reload` | — | 刷新页面 |
| `search` | `query` (必填), `engine` (可选, 默认 duckduckgo) | 搜索引擎搜索 |

### 交互类

| 动作 | 参数 | 说明 |
|---|---|---|
| `click` | `index` (必填) | 点击指定索引的元素 |
| `input` | `index` (必填), `text` (必填), `clear` (可选) | 向元素输入文本 |
| `scroll` | `direction` (可选, 默认 down), `amount` (可选) | 滚动页面 |
| `send_keys` | `keys` (必填) | 发送按键（如 `Enter`, `Control+a`） |
| `select_option` | `index` (必填), `value` (必填) | 选择下拉选项 |

### 页面类

| 动作 | 参数 | 说明 |
|---|---|---|
| `screenshot` | `index` (可选) | 截取页面/元素截图 |
| `extract` | `query` (可选) | 提取页面文本内容 |
| `switch_tab` | `index` (必填) | 切换标签页 |
| `close_tab` | `index` (可选) | 关闭标签页 |
| `new_tab` | `url` (可选) | 打开新标签页 |
| `evaluate` | `code` (必填) | 执行 JavaScript |

### 文件类

| 动作 | 参数 | 说明 |
|---|---|---|
| `save_to_file` | `file_name` (必填), `content` (必填) | 保存文件 |
| `read_file` | `file_name` (必填) | 读取文件 |

### 元动作

| 动作 | 参数 | 说明 |
|---|---|---|
| `wait` | `seconds` (可选, 默认 3, 最大 30) | 等待 |
| `done` | `text` (必填), `success` (可选) | 标记任务完成 |

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
| `FA_BROWSER_USER_DATA_DIR` | — | 用户数据目录 |

## TUI 测试器

```bash
fairy-action tester
# 或
cargo run --bin fa-tester
```

启动后进入交互式终端界面，支持以下命令：

### 命令列表

| 命令 | 用法 | 说明 |
|---|---|---|
| `navigate` | `navigate https://example.com` | 导航到 URL |
| `back` | `back` | 浏览器后退 |
| `forward` | `forward` | 浏览器前进 |
| `reload` | `reload` | 刷新页面 |
| `click` | `click 5` | 点击指定索引的元素 |
| `input` | `input 3 hello world` | 在元素中输入文本 |
| `scroll` | `scroll down 500` | 滚动页面（up/down + 像素） |
| `press` | `press Enter` | 发送按键 |
| `eval` | `eval document.title` | 执行 JavaScript |
| `tab-new` | `tab-new https://example.com` | 打开新标签页 |
| `tab-switch` | `tab-switch 0` | 切换到指定标签页 |
| `tab-close` | `tab-close 1` | 关闭指定标签页 |
| `screenshot` | `screenshot` | 截取屏幕截图 |
| `dom` | `dom` | 刷新 DOM 树 |
| `find` | `find 关键词` | 在页面中搜索文本 |
| `url` | `url` | 显示当前 URL |
| `title` | `title` | 显示页面标题 |
| `clear` | `clear` | 清除日志 |
| `help` | `help` | 切换帮助面板 |
| `quit` | `quit` | 退出 |

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
| `Shift + Up/Down` | DOM 树逐行滚动 |
| `Shift + Home/End` | DOM 树跳到顶部/底部 |
| `Esc` | 清空命令输入 |

### TUI 界面布局

```
+--------------------------------------------------------------------+
|  FairyAction Tester  https://example.com | Page Title               |
+----------------------------------------+---------------------------+
|                                        |  Status                    |
|  DOM Tree (F5 refresh, PgUp/PgDn)     |  URL: ...                  |
|                                        |  Title: ...                |
|  [0] <a href="/home"> "Home"          |  Tab: 1/3                  |
|  [1] <button> "Submit"                |                            |
|  [2] input placeholder="Search"       +---------------------------+
|  [3] <a href="/about"> "About"        |  Log                       |
|  ...                                   |  > Clicked element [1]     |
|                                        |  > Navigated to: ...       |
|                                        |  > DOM refreshed: 42       |
|                                        |  > interactive elements    |
+----------------------------------------+---------------------------+
|  Command (type 'help' for commands) > _                          |
+--------------------------------------------------------------------+
```

## 项目结构

```
FairyAction/
+-- crates/
|   +-- fa-cli/        # CLI 入口 (clap), JSON 协议接口
|   +-- fa-tester/     # TUI 测试器 (ratatui + crossterm)
|   +-- fa-browser/    # 浏览器控制 (CDP WebSocket)
|   +-- fa-dom/        # DOM 解析与序列化
|   +-- fa-actor/      # 页面元素操作抽象 (Page/Element/Mouse)
|   +-- fa-tools/      # 动作注册表与执行引擎
|   +-- fa-config/     # 配置管理
+-- .env               # 环境变量配置
+-- Cargo.toml         # Workspace 根配置
```

### 依赖关系

```
fa-cli
  +-- fa-config
  +-- fa-browser
  +-- fa-dom
  +-- fa-tools
      +-- fa-browser
      +-- fa-actor
      +-- fa-dom

fa-tester
  +-- fa-config
  +-- fa-browser
  +-- fa-dom
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
