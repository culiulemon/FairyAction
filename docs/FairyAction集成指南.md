# FairyAction 集成指南

> 将 FairyAction 接入你的 AI Agent 产品，实现 FAP 包管理和触桥协议交互

## 目录

- [概述](#概述)
- [架构总览](#架构总览)
- [集成模式](#集成模式)
- [触桥协议](#触桥协议)
- [FAP 包系统](#fap-包系统)
- [配置系统](#配置系统)
- [集成实战（以 AuroraFairy 为例）](#集成实战以-aurorafairy-为例)
- [常见问题与踩坑记录](#常见问题与踩坑记录)

---

## 概述

FairyAction 是一个 AI Agent 能力编排平台。它将浏览器自动化、文件操作、系统工具等能力封装为统一的「触桥协议」接口，供 AI Agent 通过 stdin/stdout 通信调用。核心扩展机制是 **FAP (FairyAction Package)** 生态——将任意 CLI 工具或自定义程序封装为 AI Agent 可调用的能力单元。

### 核心概念

| 概念 | 说明 |
|------|------|
| **FAP 包** | FairyAction Package，一个 `.fap` 文件（ZIP 格式），包含 manifest.json + 二进制/脚本 |
| **触桥协议** | 宿主与 FairyAction 之间的通信协议，帧格式 + URI 风格消息 |
| **Bridge 模式** | `fairy-action bridge` 子进程模式，通过帧协议管理 FAP 包并调用动作 |
| **Manifest 模式** | 零代码模式，只需编写 manifest.json 即可将 CLI 工具封装为 FAP 包 |
| **SDK 模式** | 使用 `fa-bridge-sdk` crate 编写 Rust 程序，支持更复杂的交互逻辑 |

### Crate 结构

| Crate | 职责 |
|-------|------|
| `fa-cli` | CLI 入口、交互模式、bridge 模式 |
| `fa-bridge` | 触桥协议引擎：消息封装、帧协议 |
| `fa-bridge-sdk` | 开发者 SDK（独立 crate，可发布到 crates.io） |
| `fa-fap` | FAP 包管理：manifest、invoke 渲染、签名、打包、解析器、进程池 |
| `fa-tools` | 动作注册表与执行引擎 + FapManager |
| `fa-config` | 配置管理 + FapConfig |

---

## 架构总览

```
┌───────────────────────────────────────────────────────────────┐
│                     宿主产品 (你的应用)                          │
│                                                               │
│  ┌──────────────┐    ┌────────────────────────────────────┐   │
│  │  管理界面 UI  │    │  AI Agent 循环                       │   │
│  └──────┬───────┘    │  └→ 触桥工具 → fap_bridge_send()    │   │
│         │            └─────────────┬──────────────────────┘   │
│         │ invoke                  │ invoke                    │
│         ▼                         ▼                           │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │              后端 (Tauri / Electron / ...)               │  │
│  │                                                         │  │
│  │  BridgeManager                                          │  │
│  │  ├── bridge_start()  → 启动 fairy-action bridge 子进程   │  │
│  │  ├── bridge_send()   → 发送触桥帧，接收响应帧             │  │
│  │  └── bridge_stop()   → 关闭子进程                        │  │
│  │                                                         │  │
│  │  FAP 管理命令 (CLI 子进程调用)                            │  │
│  │  ├── fap_install()   → fairy-action fap install          │  │
│  │  ├── fap_uninstall() → fairy-action fap uninstall        │  │
│  │  └── fap_list()      → fairy-action fap list + inspect   │  │
│  └──────────────────────┬──────────────────────────────────┘  │
│                         │ stdin/stdout 帧协议                   │
│                         ▼                                      │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │          fairy-action bridge 子进程                       │ │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────────────┐ │ │
│  │  │ FapManager │  │ ProcessPool│  │ Invoke 渲染引擎    │ │ │
│  │  │ (包管理)    │  │ (SDK进程池) │  │ (零代码模式)       │ │ │
│  │  └────────────┘  └────────────┘  └────────────────────┘ │ │
│  └──────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────┘
```

**关键设计**：
- 宿主产品通过子进程方式与 FairyAction 通信，不需要链接 Rust 库
- Bridge 子进程是常驻的，管理所有 FAP 包的生命周期
- CLI 子进程是一次性的，用于安装/卸载等管理操作
- 所有通信都通过 stdin/stdout 管道，跨语言、跨平台

---

## 集成模式

### 模式一：子进程 Bridge 模式（推荐）

这是最通用的集成方式，适用于任何语言和技术栈的产品。

**原理**：启动 `fairy-action bridge` 作为常驻子进程，通过帧协议交换触桥消息。

```
宿主 ←→ stdin/stdout 管道 ←→ fairy-action bridge 子进程
```

**适用场景**：AI Agent 产品需要动态调用 FAP 包中的动作。

### 模式二：CLI 命令调用

**原理**：通过 `fairy-action fap install/uninstall/list/inspect` 等命令进行包管理。

```
宿主 → 启动 CLI 子进程 → 等待完成 → 读取 stdout
```

**适用场景**：FAP 包的安装、卸载、列表查看等管理操作。

### 模式三：Rust 库集成

**原理**：直接依赖 `fa-fap`、`fa-tools`、`fa-bridge` 等 crate。

**适用场景**：宿主产品本身是 Rust 项目，需要更紧密的集成。

---

## 触桥协议

### 帧格式

所有 bridge 通信都使用帧格式：

```
<length> <message>\n
```

| 字段 | 说明 |
|------|------|
| `<length>` | 消息体的字节长度（十进制 ASCII） |
| ` ` (空格) | 长度和消息体的分隔符 |
| `<message>` | UTF-8 编码的消息字符串 |
| `\n` | 帧结束标记 |

**示例**：消息 `bridge://hello#`（15 字节）编码为 `15 bridge://hello#\n`

#### 写帧伪代码

```python
def write_frame(stdin, message):
    frame = f"{len(message)} {message}\n"
    stdin.write(frame)
    stdin.flush()
```

#### 读帧伪代码

```python
def read_frame(stdout):
    line = stdout.readline()
    trimmed = line.strip()
    space_pos = trimmed.find(' ')
    length = int(trimmed[:space_pos])
    message = trimmed[space_pos + 1:]
    return message
```

### 消息格式

触桥消息使用 URI 风格格式：

```
bridge://<type>\x1F<module>\x1F<channel>\x1F<action>#<JSON_PAYLOAD>
```

| 字段 | 分隔符 | 说明 |
|------|--------|------|
| `<type>` | 开头 | 消息类型：`hello` / `call` / `ok` / `error` / `progress` / `configure` |
| `<module>` | `\x1F` | FAP 包标识符（如 `com.ffmpeg.fap`） |
| `<channel>` | `\x1F` | 能力域名称 |
| `<action>` | `\x1F` | 动作名称 |
| `<JSON>` | `#` | JSON 格式的载荷 |

> `\x1F` 是 Unit Separator (ASCII 0x1F)

### 消息类型

#### Hello — 能力查询

查询已安装 FAP 包的能力描述。

**查询全部包**：
```
bridge://hello#
```

**查询特定包**：
```
bridge://hello\x1Fcom.ffmpeg.fap#
```

**响应**（ok 类型）：
```
bridge://ok\x1Fhello#{"modules":{"com.ffmpeg.fap":{"能力域":[...]}}}
```

查询单个包时：
```
bridge://ok\x1Fhello#{"module":"com.ffmpeg.fap","capabilities":{"能力域":[...]}}
```

#### Call — 调用动作

**请求**：
```
bridge://call\x1Fcom.ffmpeg.fap\x1F音频处理\x1F音频转mp3#{"输入":"demo.flac","输出":"demo.mp3","比特率":"192k"}
```

**成功响应**：
```
bridge://ok\x1Fcom.ffmpeg.fap\x1F音频转mp3#{"output":"...","extracted_content":"..."}
```

**失败响应**：
```
bridge://error\x1Fcom.ffmpeg.fap\x1F音频转mp3#{"错误码":"...","错误信息":"..."}
```

#### Configure — 运行时配置

**请求**：
```
bridge://configure#{"fap.refresh_manifests":true}
```

用于安装/卸载 FAP 包后刷新 bridge 进程的 manifest 缓存。

**支持的配置键**：

| 键 | 说明 |
|----|------|
| `fap.install_dir` | 安装目录（变更后自动刷新 manifest） |
| `fap.temp_dir` | 临时目录 |
| `fap.host_data_dir` | 宿主数据目录 |
| `fap.default_timeout` | 默认超时（秒） |
| `fap.max_concurrent` | 最大并发数 |

### 响应解析

响应帧的消息格式与请求相同。解析步骤：

1. 从帧中提取消息字符串
2. 找到 `#` 分隔符
3. `#` 之前的部分解析消息类型和路由字段
4. `#` 之后的部分解析为 JSON

**关键点**：bridge 模式会将 `ActionResult` 的 `output` 字段（JSON 字符串）解析为 `Value` 后放入响应载荷。如果 `output` 不是有效 JSON，则包装为 `{"output": "..."}`。

---

## FAP 包系统

### 包结构

一个 FAP 包是一个 ZIP 文件（`.fap` 后缀），内部结构：

```
com.ffmpeg.fap/              ← 包目录（ZIP 根目录）
├── manifest.json            ← 必须，包描述文件
├── signature.sig            ← 可选，Ed25519 签名
├── bin/
│   └── windows-x86_64/
│       ├── ffmpeg.exe       ← 入口二进制
│       └── ffprobe.exe
└── resources/
    └── ...
```

> **重要**：`manifest.json` 必须位于 ZIP 根目录，不能嵌套在子目录中。打包时使用 `fairy-action fap pack` 命令可确保正确格式。

### Manifest 格式

`manifest.json` 是 FAP 包的核心描述文件：

```json
{
    "format_version": 1,
    "package": "com.example.mytool",
    "name": "我的工具",
    "version": "1.0.0",
    "description": "工具描述",
    "mode": "manifest",
    "lifecycle": "oneshot",
    "platforms": ["windows-x86_64", "linux-x86_64"],
    "entry": {
        "windows-x86_64": "bin/windows-x86_64/mytool.exe",
        "linux-x86_64": "bin/linux-x86_64/mytool"
    },
    "capabilities": {
        "core": [
            {
                "名称": "核心功能",
                "动作": [
                    {
                        "名称": "处理文件",
                        "参数": {
                            "输入": {
                                "类型": "string",
                                "必填": true,
                                "描述": "输入文件路径"
                            },
                            "质量": {
                                "类型": "integer",
                                "默认": 85,
                                "描述": "处理质量 1-100"
                            }
                        },
                        "invoke": {
                            "args": ["-i", "{{输入}}", "-q", "{{质量}}", "{{输出}}"],
                            "output": {
                                "source": "stderr",
                                "parser": "last_line"
                            },
                            "timeout": 120
                        }
                    }
                ]
            }
        ]
    },
    "permissions": ["filesystem.read", "filesystem.write"]
}
```

### 字段说明

#### 顶层字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `format_version` | `u32` | 是 | 格式版本，当前为 `1` |
| `package` | `String` | 是 | 反向域名格式包标识符（如 `com.ffmpeg.fap`） |
| `name` | `String` | 是 | 人类可读包名 |
| `version` | `String` | 是 | 语义化版本号（`x.y.z`） |
| `description` | `String` | 否 | 包描述 |
| `mode` | `"manifest"` / `"sdk"` | 是 | 开发模式 |
| `lifecycle` | `"oneshot"` / `"persistent"` / `"both"` | 否 | 生命周期 |
| `platforms` | `String[]` | 是 | 支持的平台列表 |
| `entry` | `Map<String, String>` | 是 | 平台到入口二进制的映射 |
| `capabilities` | `Map<String, CapabilityDomain[]>` | manifest 模式必填 | 能力域 |
| `permissions` | `String[]` | 否 | 权限声明 |
| `signature` | `SignatureInfo` | 否 | 签名信息 |

#### PackageMode

| 值 | 说明 |
|----|------|
| `"manifest"` | 零代码模式：只需编写 manifest.json，通过 invoke 配置映射 CLI 调用 |
| `"sdk"` | SDK 模式：使用 `fa-bridge-sdk` 编写 Rust 程序 |

#### Lifecycle

| 值 | 说明 |
|----|------|
| `"oneshot"` | 每次调用启动新进程，调用完毕退出 |
| `"persistent"` | 长驻进程，通过 stdin/stdout 消息协议通信 |
| `"both"` | 两种模式都支持（由调用方决定） |

#### 平台标识

格式：`<os>-<arch>`

| 平台 | 标识 |
|------|------|
| Windows x64 | `windows-x86_64` |
| Linux x64 | `linux-x86_64` |
| macOS Apple Silicon | `macos-aarch64` |
| macOS Intel | `macos-x86_64` |

#### 合法权限

| 权限 | 说明 |
|------|------|
| `filesystem.read` | 读取文件系统 |
| `filesystem.write` | 写入文件系统 |
| `network.outbound` | 出站网络访问 |
| `process.spawn` | 启动子进程 |
| `clipboard.read` | 读取剪贴板 |
| `clipboard.write` | 写入剪贴板 |

### Manifest 验证规则

1. `package` 不能为空
2. `platforms` 不能为空
3. `entry` 至少包含一个平台
4. `entry` 中的每个平台必须出现在 `platforms` 中
5. `mode` 为 `manifest` 时 `capabilities` 不能为空
6. 所有 `permissions` 必须是合法权限

### Invoke 配置（零代码模式核心）

`invoke` 配置定义了动作如何映射到命令行调用。

#### 关键理解

**`entry` 指定可执行二进制文件**，`invoke.args` 是传给该二进制的**命令行参数**。

```
实际执行 = entry 二进制 + invoke.args 渲染后的参数
```

例如，entry 为 `bin/ffmpeg.exe`，invoke.args 为 `["-i", "{{输入}}", "{{输出}}"]`，则执行：

```
/path/to/fap/com.ffmpeg.fap/bin/ffmpeg.exe -i demo.flac demo.mp3
```

#### InvokeConfig 字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `args` | `String[]` | 参数模板数组，支持 `{{变量名}}` 模板 |
| `env` | `Map<String, String>` | 环境变量 |
| `exit_code` | `Map<String, String>` | 退出码映射（如 `"1": "error"`） |
| `output` | `OutputConfig` | 输出配置 |
| `timeout` | `u32` | 超时秒数 |

#### 模板变量

| 变量 | 解析为 |
|------|--------|
| `{{参数名}}` | 从调用时传入的参数中取值 |
| `{{临时目录}}` | 系统临时目录 |
| `{{包目录}}` | 已安装的 FAP 包目录路径 |
| `{{宿主数据目录}}` | 宿主数据目录（需配置 `--fap-host-data-dir`） |
| `{{来源路径的父目录}}` | 包目录的父目录（即安装目录） |

#### 参数类型展开

| 参数 JSON 类型 | 展开方式 |
|----------------|----------|
| `String` | 单个字符串参数 |
| `Number` | 转为字符串的单个参数 |
| `Bool` | `"true"` / `"false"` 字符串参数 |
| `Array` | **展开为多个独立参数** |

#### OutputConfig 字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `source` | `"stdout"` / `"stderr"` | `"stdout"` | 从哪个流读取输出 |
| `parser` | `String` | — | 输出解析器 |
| `pattern` | `String` | — | 正则 pattern（regex 解析器用） |

#### 输出解析器

| 解析器 | 说明 |
|--------|------|
| `raw` | 原始文本，包装为 `{"output": "..."}` |
| `json` | 直接解析为 JSON |
| `last_line` | 取最后一行 |
| `lines` | 按行分割为 `{"lines": [...]}` |
| `csv` | CSV 解析为 `{"rows": [[...], ...]}` |
| `ffmpeg_progress` | FFmpeg 进度解析 |
| `regex` | 正则匹配，需提供 `pattern` |

### SDK 模式

使用 `fa-bridge-sdk` crate 编写 Rust 程序：

```rust
use fa_bridge_sdk::{App, Domain, Action, Param, Lifecycle};
use serde_json::Value;

fn main() {
    let app = App::new()
        .name("my-tool")
        .version("1.0.0")
        .lifecycle(Lifecycle::Both)
        .domain(
            Domain::new("图片转换")
                .action(
                    Action::new("png2jpg", |params, ctx| {
                        let input = params["输入"].as_str().unwrap();
                        ctx.progress(50, "处理中")?;
                        Ok(Value::String(format!("converted: {}", input)))
                    })
                    .param(Param::string("输入").required().desc("输入文件"))
                    .param(Param::int("质量").default_val(Value::from(85)))
                )
        );
    app.run();
}
```

SDK 程序支持三种运行模式（由 `app.run()` 自动选择）：
- `--capabilities`：输出能力描述 JSON 并退出
- `--serve`：persistent 模式，从 stdin 读取消息循环
- `<action> [--param value...]`：oneshot 模式

### 签名系统

FAP 包支持 Ed25519 签名验证。

**签名流程**：

```bash
# 1. 生成密钥对
fairy-action fap keygen --output ./keys

# 2. 签名包目录
fairy-action fap sign --key ./keys/fap_private.key --package ./my-tool

# 3. 打包（自动包含签名）
fairy-action fap pack --package ./my-tool --output ./dist

# 4. 验证签名
fairy-action fap verify --package ./my-tool
```

**签名算法**：
1. 收集包目录下所有文件（排除 `signature.sig` 和 `manifest.json`）
2. 按相对路径字典序排序
3. 每个文件 SHA-256 哈希
4. 拼接 `路径:哈希\n` 字符串
5. 对拼接字符串 SHA-256 得到摘要
6. Ed25519 签名摘要

---

## 配置系统

### 配置文件

默认路径：`{系统配置目录}/fairy-action/config.json`

```json
{
    "browser": {
        "headless": false,
        "viewport_width": 1280,
        "viewport_height": 720
    },
    "fap": {
        "install_dir": "/path/to/fap",
        "temp_dir": "/tmp",
        "host_data_dir": "/path/to/host/data",
        "default_timeout": 60,
        "max_concurrent": 4
    }
}
```

### 环境变量

所有配置项都可通过环境变量覆盖：

| 环境变量 | 说明 |
|----------|------|
| `FA_FAP_INSTALL_DIR` | FAP 安装目录 |
| `FA_FAP_TEMP_DIR` | 临时文件目录 |
| `FA_FAP_HOST_DATA_DIR` | 宿主数据目录 |
| `FA_FAP_DEFAULT_TIMEOUT` | 默认超时（秒） |
| `FA_FAP_MAX_CONCURRENT` | 最大并发数 |

### 默认路径

| 路径 | 默认值 |
|------|--------|
| 安装目录 | `{系统数据目录}/fairy-action/fap` |
| 临时目录 | `{系统临时目录}` |
| 配置文件 | `{系统配置目录}/fairy-action/config.json` |

> **Windows 上的数据目录**：`C:\Users\{用户名}\AppData\Roaming\fairy-action\fap`

---

## 集成实战（以 AuroraFairy 为例）

以下展示了一个完整的集成流程，基于 AuroraFairy（Tauri v2 + Vue 3）的真实实现。

### 第一步：确定集成架构

AuroraFairy 采用了双子进程架构：

| 进程 | 启动方式 | 用途 | 通信协议 |
|------|----------|------|----------|
| `fairy-action bridge` | 常驻子进程 | FAP 包动作调用 | 帧协议（stdin/stdout） |
| `fairy-action fap ...` | 一次性子进程 | 包安装/卸载/查看 | CLI stdout |

### 第二步：实现 Bridge 进程管理

后端需要管理 bridge 子进程的生命周期：

```rust
// Rust (Tauri 后端示例)
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct BridgeManager {
    child: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    stdout: Arc<Mutex<Option<BufReader<ChildStdout>>>>,
}

// 启动 bridge
async fn bridge_start(app: &AppHandle, state: &State<'_, BridgeManager>) -> Result<(), String> {
    let exe_path = find_fairy_action_binary(app)?;
    let fap_install_dir = app.path().app_data_dir()?.join("fap");

    let mut cmd = Command::new(&exe_path);
    cmd.arg("bridge")
        .arg("--fap-install-dir").arg(&fap_install_dir)
        .arg("--fap-host-data-dir").arg(app.path().app_data_dir()?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Windows 下隐藏控制台窗口
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let mut child = cmd.spawn().await?;
    // 保存 stdin/stdout 管道到 BridgeManager...
    Ok(())
}
```

### 第三步：实现帧协议读写

```rust
async fn write_frame(stdin: &Arc<Mutex<Option<ChildStdin>>>, message: &str) -> Result<(), String> {
    let frame = format!("{} {}\n", message.len(), message);
    let mut guard = stdin.lock().await;
    let handle = guard.as_mut().ok_or("bridge 未启动")?;
    handle.write_all(frame.as_bytes()).await?;
    handle.flush().await?;
    Ok(())
}

async fn read_frame(stdout: &Arc<Mutex<Option<BufReader<ChildStdout>>>>>) -> Result<String, String> {
    let mut guard = stdout.lock().await;
    let reader = guard.as_mut().ok_or("bridge 未启动")?;
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let trimmed = line.trim();
    let space_pos = trimmed.find(' ').ok_or("帧格式错误")?;
    Ok(trimmed[space_pos + 1..].to_string())
}
```

### 第四步：实现 FAP 包管理

包管理通过 CLI 子进程实现，**必须确保安装目录与 bridge 一致**：

```rust
async fn run_fap_cli(app: &AppHandle, args: Vec<&str>) -> Result<String, String> {
    let exe_path = find_fairy_action_binary(app)?;
    let fap_install_dir = app.path().app_data_dir()?.join("fap");

    let mut cmd = Command::new(&exe_path);
    cmd.args(&args)
        .env("FA_FAP_INSTALL_DIR", &fap_install_dir)  // 关键！统一安装目录
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().await?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

### 第五步：实现 Agent 工具集成

前端虚拟处理器将 Agent 的工具调用转化为触桥消息：

```typescript
// TypeScript (前端虚拟处理器)
async function handleFapBridge(input: any): Promise<string> {
    const action = input.action

    // 自动确保 bridge 已启动
    async function ensureBridgeAndSend(message: string): Promise<string> {
        try {
            const result = await invoke('fap_bridge_send', { message })
            return JSON.stringify(result, null, 2)
        } catch (e) {
            if (String(e).includes('触桥未启动')) {
                await invoke('fap_bridge_start')  // 自动启动
                const retry = await invoke('fap_bridge_send', { message })
                return JSON.stringify(retry, null, 2)
            }
            return JSON.stringify({ success: false, error: String(e) })
        }
    }

    if (action === 'list') {
        const result = await invoke('fap_list')
        return JSON.stringify(result, null, 2)
    }

    if (action === 'call') {
        const msg = `bridge://call\x1F${input.module}\x1F${input.channel}\x1F${input.fap_action}#${input.params || '{}'}`
        return await ensureBridgeAndSend(msg)
    }
}
```

### 第六步：注入系统提示

让 AI Agent 在对话开始时就知道可用的 FAP 能力：

```typescript
async function assembleFapPrompt(): Promise<string> {
    const result = await invoke('fap_list')
    const packages = result?.packages || []
    if (packages.length === 0) return ''

    const lines = ['## 触桥应用 (FAP)', '', '以下是已安装的 FAP 应用：', '']
    for (const pkg of packages) {
        lines.push(`### ${pkg.name} (${pkg.package}) v${pkg.version}`)
        // 遍历能力域和动作...
    }
    return lines.join('\n')
}
```

---

## 常见问题与踩坑记录

### 1. FAP 包安装后 bridge 找不到包

**症状**：`fap_list` 能看到包，但 bridge 的 hello/call 返回 `FAP package not found`。

**原因**：CLI 安装命令和 bridge 进程使用了不同的安装目录。CLI 默认用 `dirs::data_dir()/fairy-action/fap/`，bridge 用 `--fap-install-dir` 指定的路径。

**解决**：确保所有 CLI 调用通过环境变量 `FA_FAP_INSTALL_DIR` 指定统一的安装目录，与 bridge 的 `--fap-install-dir` 参数一致。

```rust
cmd.env("FA_FAP_INSTALL_DIR", &fap_install_dir);
```

### 2. manifest.json not found in archive

**症状**：安装 `.fap` 包时报错 `manifest.json not found in archive`。

**原因**：FAP 包的 ZIP 内部有一层额外目录嵌套（如 `com.ffmpeg.fap/manifest.json` 而非根目录的 `manifest.json`）。

**解决**：
- 方案 A：使用 `fairy-action fap pack` 命令打包，确保正确格式
- 方案 B：在安装前预处理 ZIP，剥离外层目录

### 3. invoke.args 中重复了 entry 路径

**症状**：FAP 动作调用时传入了错误的命令行参数。

**原因**：在 `invoke.args` 的第一个元素中写了可执行文件路径（如 `"{{来源路径的父目录}}/bin/ffmpeg.exe"`），但 `entry` 字段已经指定了可执行文件。

**正确做法**：`invoke.args` 只放传给 entry 二进制的**命令行参数**，不要包含可执行文件路径本身。

```json
// 错误 ❌
"invoke": {
    "args": ["{{来源路径的父目录}}/bin/ffmpeg.exe", "-i", "{{输入}}"]
}

// 正确 ✓
"invoke": {
    "args": ["-i", "{{输入}}", "{{输出}}"]
}
```

### 4. bridge 进程未启动导致调用失败

**症状**：Agent 调用 `hello` 或 `call` 时返回 "触桥未启动"。

**原因**：bridge 进程是按需启动的，Agent 不会主动调用 `bridge_start`。

**解决**：在虚拟处理器中实现自动启动逻辑——先尝试发送消息，如果失败且错误包含"未启动"，则自动启动 bridge 后重试。

### 5. 安装/卸载后 bridge 缓存未刷新

**症状**：安装了新包但 bridge 的 hello 查询不到。

**原因**：bridge 进程启动时会加载 manifest 缓存，安装新包后缓存未更新。

**解决**：安装/卸载后向 bridge 发送 configure 消息刷新缓存：

```
bridge://configure#{"fap.refresh_manifests":true}
```

### 6. Windows 下子进程弹出控制台窗口

**解决**：在 Windows 上启动子进程时添加 `CREATE_NO_WINDOW` 标志：

```rust
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

cmd.creation_flags(CREATE_NO_WINDOW);
```

---

## CLI 命令速查

### FAP 包管理

```bash
# 安装
fairy-action fap install <path> [--skip-verify]

# 卸载
fairy-action fap uninstall <package>

# 列出已安装的包
fairy-action fap list

# 查看包详情（输出完整 manifest JSON）
fairy-action fap inspect <package>

# 直接运行动作
fairy-action fap run <package> <capability> <action> [json_params] \
    [--fap-install-dir <dir>] [--fap-temp-dir <dir>] [--fap-host-data-dir <dir>]
```

### 签名与打包

```bash
# 生成密钥对
fairy-action fap keygen [--output <dir>]

# 签名
fairy-action fap sign --key <private_key_path> --package <dir>

# 打包
fairy-action fap pack --package <dir> [--output <path>] [--verify]

# 验证签名
fairy-action fap verify --package <dir>
```

### Bridge 模式

```bash
# 启动 bridge 子进程（从 stdin 读取帧消息）
fairy-action bridge \
    [--fap-install-dir <dir>] \
    [--fap-temp-dir <dir>] \
    [--fap-host-data-dir <dir>]
```

### 浏览器自动化

```bash
# 启动浏览器自动化（从 stdin 读取 JSON 请求）
fairy-action run [--show-browser] [--log-level <level>]

# 列出所有可用动作
fairy-action list-actions
```

### 配置

```bash
# 显示当前配置
fairy-action config show

# 初始化配置文件
fairy-action config init

# 设置配置项
fairy-action config set <key> <value>
```
