# FairyAction

AI Agent 的浏览器自动化基础设施，使用 Rust 构建。通过 JSON 协议暴露精细的浏览器控制接口，供外部 AI Agent 调用。

## 特性

- **JSON 协议接口** — stdin/stdout 交互，外部 Agent 发送 JSON 指令即可控制浏览器
- **21 个内置动作** — 导航、点击、输入、滚动、标签页管理、文件操作、标注覆盖等
- **智能 DOM 提取** — 层级化 DOM 表示，区分可交互/不可交互元素，中文类型名
- **交互性检测** — 自动识别按钮、链接、输入框等可交互元素，仅对可交互元素分配操作索引
- **元素标注覆盖层** — 在浏览器中以线框标注所有可交互元素的位置与索引
- **路由变化感知** — 自动检测 URL 变化并刷新 DOM 树
- **真实鼠标事件** — 模拟真实用户点击，支持 Vue/React 等前端框架
- **动作状态追踪** — 执行动作后返回完整的页面状态快照（URL、标题、标签页数、导航检测等）
- **多标签页管理** — 自动检测新标签页、切换 CDP 连接
- **URL 自动补全** — 自动为缺少协议的 URL 补全 `https://`
- **浏览器身份** — 默认窗口标题 "FairyBrowser"，默认用户 Profile "Fairy"
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
  -v, --verbose           启用详细日志（输出到 stderr）
  -q, --quiet             静默模式，不输出任何日志
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
fairy-action run --quiet --show-browser  # 作为子进程集成时使用（无日志干扰）
```

启动后通过 **stdin** 接收 JSON 请求，通过 **stdout** 返回 JSON 响应。所有日志输出到 **stderr**，不会污染 JSON 通信管道。

### 请求格式

```jsonc
// 执行动作
{"type": "execute", "action": "navigate", "params": {"url": "https://example.com"}}

// 获取浏览器状态（含视口和滚动位置）
{"type": "get_state"}

// 获取当前页面 DOM（show_empty 为 true 时显示空区块）
{"type": "get_dom"}
{"type": "get_dom", "show_empty": true}

// 列出可用动作
{"type": "list_actions"}

// 切换元素标注覆盖层（show 为 true/false/null）
{"type": "toggle_annotations"}
{"type": "toggle_annotations", "show": true}

// 关闭浏览器
{"type": "close"}
```

### 响应格式

```jsonc
// 动作执行成功
{
  "type": "ok",
  "action": "click",
  "result": {
    "success": true,
    "output": "Clicked element [5]",
    "is_done": false,
    "state_after": {
      "url": "https://example.com/page",
      "title": "Page Title",
      "tab_count": 1,
      "new_tab_opened": true,
      "navigation_occurred": true
    }
  }
}

// 动作执行失败
{"type": "ok", "action": "click", "result": {"success": false, "error": "Element with index 99 not found"}}

// 浏览器状态（含视口和滚动信息）
{"type": "state", "url": "https://example.com", "title": "Example", "viewport": {"width": 1280, "height": 720}, "scroll": {"x": 0, "y": 300}, "tabs": [...]}

// DOM 内容（层级化表示）
{"type": "dom", "url": "https://example.com", "title": "Example", "representation": "...", "element_count": 42}

// 动作列表
{"type": "actions", "actions": [...], "schema": {...}}

// 错误
{"type": "error", "message": "Unknown action: 'xxx'"}

// 关闭确认
{"type": "closed"}
```

### 集成示例

外部 AI Agent 的典型集成流程：

```bash
# 启动 FairyAction（作为子进程，--quiet 禁用日志，--show-browser 显示浏览器）
fairy-action run --quiet --show-browser
```

```
1. 通过 stdin 发送 {"type": "list_actions"} 获取可用动作
2. 通过 stdin 发送 {"type": "get_dom"} 获取页面 DOM
3. 根据任务需求，发送 {"type": "execute", "action": "click", "params": {"index": 5}} 执行操作
4. 从 stdout 读取 JSON 响应，检查 state_after 判断操作效果
5. 重复 2-4 直到任务完成
6. 发送 {"type": "close"} 关闭浏览器
```

## DOM 表示格式

DOM 以层级化文本表示，使用 2 空格缩进体现包含关系：

### 元素分类

- **可交互元素**（带 `[index]`）— 按钮输入框等，AI 可通过 index 直接操作
- **语义元素**（无 index）— 标题段落等，只读展示，帮助 AI 理解页面上下文

### 输出示例

```
[0] 导航：首页 产品 关于
  [1] 按钮：登录
  [2] 按钮：注册
标题：欢迎使用 FairyAction
段落：AI Agent 的浏览器自动化基础设施
  [5] 输入框：搜索关键词
  [6] 按钮：搜索
表格：价格列表
  [7] 按钮：查看详情
  [8] 按钮：加入购物车
```

### 类型名称映射

| HTML 标签 | 显示名 | HTML 标签 | 显示名 |
|-----------|--------|-----------|--------|
| `a` | 链接 | `input` | 输入框 |
| `button` | 按钮 | `textarea` | 文本框 |
| `select` | 下拉框 | `option` | 选项 |
| `img` | 图片 | `video` | 视频 |
| `h1`-`h6` | 标题 | `p` | 段落 |
| `span` | 文字 | `div` | 区块 |
| `table` | 表格 | `form` | 表单 |
| `nav` | 导航 | `li` | 列表项 |
| `label` | 标签 | `dialog` | 对话框 |
| `details` | 折叠面板 | `summary` | 折叠标题 |

### 交互性判断

元素被判定为**可交互**（分配 index）的条件：
- 属于交互标签（`a`、`button`、`input`、`textarea`、`select` 等）
- 具有 `onclick` 事件处理
- `role` 属性为 `button`、`link`、`textbox` 等
- 具有 `tabindex` 属性
- CSS 样式包含 `cursor: pointer`

## 可用动作（共 21 个）

### 导航类

| 动作 | 参数 | 说明 |
|------|------|------|
| `navigate` | `url` (必填), `new_tab` (可选) | 导航到 URL（自动补全 https://） |
| `go_back` | — | 浏览器后退 |
| `go_forward` | — | 浏览器前进 |
| `reload` | — | 刷新页面 |
| `search` | `query` (必填), `engine` (可选, 默认 duckduckgo) | 搜索引擎搜索 |

### 交互类

| 动作 | 参数 | 说明 |
|------|------|------|
| `click` | `index` (必填) | 点击指定索引的元素 |
| `input` | `index` (必填), `text` (必填), `clear` (可选) | 向元素输入文本 |
| `scroll` | `direction` (可选, 默认 down), `amount` (可选), `index` (可选) | 滚动页面 |
| `send_keys` | `keys` (必填) | 发送按键（如 `Enter`, `Control+a`） |
| `select_option` | `index` (必填), `value` (必填) | 选择下拉选项 |

### 页面类

| 动作 | 参数 | 说明 |
|------|------|------|
| `screenshot` | `index` (可选) | 截取页面/元素截图 |
| `extract` | `query` (可选) | 提取页面文本内容 |
| `switch_tab` | `index` (必填) | 切换标签页 |
| `close_tab` | `index` (可选) | 关闭标签页 |
| `new_tab` | `url` (可选) | 打开新标签页（自动补全 https://） |
| `evaluate` | `code` (必填) | 执行 JavaScript |
| `toggle_annotations` | `show` (可选) | 切换元素标注覆盖层 |

### 文件类

| 动作 | 参数 | 说明 |
|------|------|------|
| `save_to_file` | `file_name` (必填), `content` (必填) | 保存文件 |
| `read_file` | `file_name` (必填) | 读取文件 |

### 元动作

| 动作 | 参数 | 说明 |
|------|------|------|
| `wait` | `seconds` (可选, 默认 3, 最大 30) | 等待 |
| `done` | `text` (必填), `success` (可选) | 标记任务完成 |

## 配置参考

### 浏览器配置

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| `FA_BROWSER_HEADLESS` | `true` | 无头模式 |
| `FA_BROWSER_VIEWPORT_WIDTH` | `1280` | 视口宽度 |
| `FA_BROWSER_VIEWPORT_HEIGHT` | `720` | 视口高度 |
| `FA_BROWSER_CHROME_PATH` | 自动检测 | Chrome 可执行文件路径 |
| `FA_BROWSER_PROXY` | — | 代理地址 |
| `FA_BROWSER_USER_DATA_DIR` | — | 用户数据目录 |
| `FA_BROWSER_PROFILE_NAME` | `Fairy` | Chrome Profile 名称 |
| `FA_BROWSER_APP_TITLE` | `FairyBrowser` | 浏览器窗口标题 |

默认使用独立的用户数据目录 `%TEMP%\FairyBrowser`，Profile 名为 "Fairy"，窗口标题为 "FairyBrowser"。

## TUI 测试器

```bash
fairy-action tester
# 或
cargo run --bin fa-tester
```

### 命令列表

| 命令 | 缩写 | 用法 | 说明 |
|------|------|------|------|
| `navigate` | `nav` | `navigate https://example.com` | 导航到 URL（自动补全协议） |
| `click` | — | `click 5` | 点击指定索引的元素 |
| `input` | — | `input 3 hello world` | 在元素中输入文本 |
| `scroll` | — | `scroll down 500` | 滚动页面（up/down + 像素） |
| `press` | `send_keys` | `press Enter` | 发送按键 |
| `screenshot` | `ss` | `screenshot` | 截取屏幕截图 |
| `back` | — | `back` | 浏览器后退 |
| `forward` | `fwd` | `forward` | 浏览器前进 |
| `reload` | `refresh` | `reload` | 刷新页面 |
| `tab-new` | — | `tab-new https://example.com` | 新建标签页（自动补全协议） |
| `tab-switch` | — | `tab-switch 0` | 切换到指定标签页 |
| `tab-close` | — | `tab-close 1` | 关闭指定标签页 |
| `eval` | `js` | `eval document.title` | 执行 JavaScript |
| `find` | — | `find 关键词` | 在页面中搜索文本 |
| `dom` | — | `dom` | 刷新 DOM 树 |
| `url` | — | `url` | 显示当前 URL |
| `title` | — | `title` | 显示页面标题 |
| `clear` | — | `clear` | 清除日志 |
| `annotate` | `ann` | `ann` | 切换元素标注覆盖层 |
| `help` | `?` | `help` | 切换帮助面板 |
| `quit` | `exit` | `quit` | 退出 |

### 快捷键

| 快捷键 | 功能 |
|--------|------|
| `F5` | 刷新 DOM 树 |
| `Ctrl+R` | 刷新页面 |
| `Ctrl+D` | 截图 |
| `Ctrl+N` | 新标签页 |
| `Ctrl+W` | 关闭标签页 |
| `Ctrl+T` | 切换元素标注覆盖层 |
| `Ctrl+C` | 退出 |
| `PageUp / PageDown` | 滚动 DOM 树视图 |
| `Shift + Up/Down` | DOM 树逐行滚动 |
| `Shift + Home/End` | DOM 树跳到顶部/底部 |
| `Enter` | 执行命令 |
| `Esc` | 清空命令输入 |

### 智能特性

- **路由变化自动刷新** — 每秒检测一次 URL，页面跳转时自动刷新 DOM 树和状态栏
- **URL 自动补全** — 输入 `navigate www.baidu.com` 会自动补全为 `https://www.baidu.com`

### TUI 界面布局

```
+--------------------------------------------------------------------+
|  FairyAction Tester  https://example.com | Page Title               |
+----------------------------------------+---------------------------+
|                                        |  Status                    |
|  DOM Tree (F5 refresh, PgUp/PgDn)     |  URL: ...                  |
|                                        |  Title: ...                |
|  [0] 导航：首页 产品                    |  Tab: 1/3                  |
|    [1] 按钮：登录                       |                            |
|    [2] 按钮：注册                       +---------------------------+
|  标题：欢迎使用                          |  Log                       |
|  段落：AI Agent 的浏览器自动化            |  > Clicked element [1]     |
|    [5] 输入框：搜索关键词                |  > URL changed: ...        |
|    [6] 按钮：搜索                       |  > DOM refreshed: 42       |
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
|------|------|
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
