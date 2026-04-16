# FairyAction

**AI Agent 能力编排平台** — 使用 Rust 构建。将浏览器自动化、文件操作、系统工具等能力封装为统一的触桥协议接口，供 AI Agent 调用。通过 FAP（FairyAction Package）生态，任何开发者都可以扩展 AI Agent 的能力边界。

## 📚 文档

- [FAP 开发者指南](docs/FAP%20开发者指南.md) — 触桥协议、manifest.json 规范、SDK API、打包工具链完整指南
- [FairyAction 集成指南](docs/FairyAction集成指南.md) — 将 FairyAction 集成到你的应用中的完整指南

## 核心架构

```
┌─────────────────────────────────────────────────────┐
│                    AI Agent / 宿主软件                 │
└──────────────────────┬──────────────────────────────┘
                       │ 触桥协议 (stdin/stdout)
                       ▼
┌─────────────────────────────────────────────────────┐
│              FairyAction 编排器 (fa-cli)              │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │
│  │ 浏览器控制 │  │ FAP 包管理 │  │  更多能力扩展...   │   │
│  │ 22个动作  │  │ 安装/调用  │  │  (第三方 FAP 包)  │   │
│  └──────────┘  └──────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────┘
```

## 特性

### 内置能力：浏览器自动化

- **JSON 协议接口** — stdin/stdout 交互，外部 Agent 发送 JSON 指令即可控制浏览器
- **22 个内置动作** — 导航、点击、输入、滚动、标签页管理、文件操作、标注覆盖等
- **智能 DOM 提取** — 层级化 DOM 表示，区分可交互/不可交互元素
- **结构化搜索结果** — 支持 Google、Bing、百度、DuckDuckGo，无需 API Key
- **多标签页管理** — 自动检测新标签页、切换 CDP 连接
- **TUI 测试器** — 交互式终端界面，实时查看 DOM 树、手动操控浏览器

### 扩展能力：FAP 生态

- **FAP 包格式** — 将任何 CLI 工具或自定义程序包装为 AI Agent 可调用的能力单元
- **零代码模式** — 通过 manifest.json 直接映射 CLI 工具参数，无需编写代码
- **SDK 模式** — 使用 [`fa-bridge-sdk`](https://crates.io/crates/fa-bridge-sdk) Rust crate 开发复杂 FAP 应用
- **触桥协议** — `bridge://<type>\x1F<module>\x1F<channel>\x1F<action>#<JSON>` 统一通信格式
- **打包工具链** — `fap keygen/sign/verify/pack` 完整的签名和发布流程
- **权限与安全** — Ed25519 签名验证、权限声明系统、版本管理

## 快速开始

### 安装

```bash
# 从源码编译
git clone https://gitcode.com/Nicek/FairyAction.git
cd FairyAction
cargo build --release

# 或从 crates.io 安装 CLI
cargo install fairy-action
```

### 浏览器自动化

```bash
# 启动浏览器交互模式
fairy-action run --show-browser

# 通过 stdin 发送 JSON 指令
{"type": "execute", "action": "navigate", "params": {"url": "https://example.com"}}
{"type": "execute", "action": "click", "params": {"index": 5}}
{"type": "get_dom"}
```

### 安装 FAP 包

```bash
# 安装一个 FAP 包
fairy-action fap install ./com.ffmpeg.fap

# 查看已安装的包
fairy-action fap list

# 调用 FAP 包中的动作
fairy-action fap run com.ffmpeg.fap 图片转换 png2jpg '{"输入":"a.png","输出":"b.jpg"}'
```

### 开发 FAP 应用

使用 `fa-bridge-sdk` 开发自定义 FAP 应用：

```bash
cargo add fa-bridge-sdk
```

```rust
use fa_bridge_sdk::{App, Domain, Action, Lifecycle};
use serde_json::Value;

fn main() {
    let app = App::new()
        .name("my-tool")
        .version("1.0.0")
        .lifecycle(Lifecycle::Oneshot)
        .domain(
            Domain::new("图片转换")
                .action(Action::new("png2jpg", |_ctx, params| {
                    let input = params["输入"].as_str().unwrap();
                    Ok(Value::String(format!("converted: {}", input)))
                }))
        );
    app.run();
}
```

> 📖 完整的开发者文档请参阅 [FAP 开发者指南](docs/FAP%20开发者指南.md)

## 平台支持

| 平台 | 架构 | 浏览器自动化 | FAP 支持 |
|------|------|-------------|---------|
| Windows | x86_64 | ✅ | ✅ |
| Windows | ARM64 | ✅ | ✅ |
| Linux | x86_64 | ✅ | ✅ |
| macOS | ARM (M1/M2/M3) | ✅ | ✅ |
| macOS | Intel | ✅ | ✅ |

## CLI 命令

```
fairy-action [OPTIONS] <COMMAND>

Commands:
  run                启动浏览器自动化（stdin/stdout JSON 协议）
  list-actions       列出所有可用动作
  tester             启动交互式 TUI 测试器
  config             配置管理
  fap                FAP 包管理子命令
  bridge             触桥协议交互模式

FAP 子命令:
  fap install        安装 FAP 包
  fap uninstall      卸载 FAP 包
  fap list           列出已安装的 FAP 包
  fap inspect        查看包详细信息
  fap run            直接运行 FAP 包中的动作
  fap keygen         生成 Ed25519 密钥对
  fap sign           签名 FAP 包
  fap verify         验证 FAP 包签名
  fap pack           打包 FAP 包目录为 .fap 文件
```

## 项目结构

```
FairyAction/
├── crates/
│   ├── fa-cli/           # CLI 入口 + 触桥协议交互模式
│   ├── fa-bridge/        # 触桥协议引擎（消息封装、帧协议、传输）
│   ├── fa-bridge-sdk/    # 开发者 SDK（独立 crate，已发布到 crates.io）
│   ├── fa-fap/           # FAP 包管理（manifest、invoke、签名、打包、权限）
│   ├── fa-browser/       # 浏览器控制 (CDP WebSocket)
│   ├── fa-dom/           # DOM 解析与序列化
│   ├── fa-actor/         # 页面元素操作抽象
│   ├── fa-tools/         # 动作注册表与执行引擎 + FapManager
│   ├── fa-config/        # 配置管理 + FapConfig
│   └── fa-tester/        # TUI 测试器
├── docs/
│   ├── FAP 开发者指南.md        # FAP 生态完整开发者指南
│   └── FairyAction集成指南.md   # 宿主软件集成指南
└── Cargo.toml
```

## 浏览器自动化详细文档

### 支持的浏览器

**Windows**: Google Chrome, Brave Browser, Microsoft Edge
**Linux**: Google Chrome, Chromium, Brave Browser, Microsoft Edge
**macOS**: Google Chrome, Chromium, Brave Browser, Microsoft Edge

### 请求格式

```jsonc
{"type": "execute", "action": "navigate", "params": {"url": "https://example.com"}}
{"type": "execute", "action": "click", "params": {"index": 5}}
{"type": "execute", "action": "input", "params": {"index": 3, "text": "hello"}}
{"type": "get_state"}
{"type": "get_dom"}
{"type": "close"}
```

### 可用动作（22 个）

| 类别 | 动作 | 说明 |
|------|------|------|
| 导航 | `navigate` `go_back` `go_forward` `reload` `search` | 页面导航和搜索引擎搜索 |
| 交互 | `click` `input` `scroll` `send_keys` `select_option` | 元素交互操作 |
| 页面 | `screenshot` `extract` `switch_tab` `close_tab` `new_tab` `evaluate` `toggle_annotations` | 页面信息和管理 |
| 文件 | `save_to_file` `read_file` | 文件读写 |
| 元动作 | `start` `wait` `done` | 状态检查和任务管理 |

### DOM 表示

```
[0] 导航：首页 产品 关于
  [1] 按钮：登录
  [2] 按钮：注册
标题：欢迎使用 FairyAction
段落：AI Agent 能力编排平台
  [5] 输入框：搜索关键词
  [6] 按钮：搜索
```

可交互元素带 `[index]`，AI Agent 可直接通过 index 操作。

### 配置

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| `FA_BROWSER_HEADLESS` | `true` | 无头模式 |
| `FA_BROWSER_CHROME_PATH` | 自动检测 | Chrome 路径 |
| `FA_BROWSER_VIEWPORT_WIDTH` | `1280` | 视口宽度 |
| `FA_BROWSER_VIEWPORT_HEIGHT` | `720` | 视口高度 |
| `FA_DEFAULT_SEARCH_ENGINE` | `bing` | 默认搜索引擎 |
| `FA_SCREENSHOT_DIR` | 系统临时目录 | 截图保存目录 |
| `FA_DOWNLOAD_DIR` | 系统临时目录 | 文件下载目录 |

```bash
fairy-action config init                          # 初始化配置
fairy-action config show                          # 查看配置
fairy-action config set browser.headless false    # 修改配置
```

### TUI 测试器

```bash
fairy-action tester
```

交互式终端界面，支持实时 DOM 查看、命令操作、快捷键控制。

## 技术栈

| 组件 | 技术 |
|------|------|
| 语言 | Rust (Edition 2024, MSRV 1.85) |
| 异步运行时 | Tokio |
| 浏览器控制 | CDP via tokio-tungstenite |
| 签名算法 | Ed25519 (ed25519-dalek) |
| TUI 框架 | ratatui + crossterm |
| CLI 解析 | clap |
| 序列化 | serde + serde_json |

## License

MIT License. Copyright (c) 2026 FairyAction.
