# FAP 开发者指南

## 目录

- [1. 概述](#1-概述)
- [2. 安装与依赖](#2-安装与依赖)
- [3. 快速开始](#3-快速开始)
- [4. 触桥协议详解](#4-触桥协议详解)
- [5. manifest.json 规范](#5-manifestjson-规范)
- [6. 开发模式](#6-开发模式)
- [7. fa-bridge-sdk API 参考](#7-fa-bridge-sdk-api-参考)
- [8. 内置解析器](#8-内置解析器)
- [9. 打包工具链](#9-打包工具链)
- [10. 权限系统](#10-权限系统)
- [11. 签名与安全](#11-签名与安全)
- [12. CLI 命令参考](#12-cli-命令参考)
- [13. 最佳实践与 FAQ](#13-最佳实践与-faq)

***

## 1. 概述

FAP（FairyAction Package）是 FairyAction 的扩展包格式，用于将原生工具包装为 AI Agent 可调用的能力单元。通过 FAP，开发者可以将任何 CLI 工具或自定义程序集成为 FairyAction 生态的一部分。

### 核心概念

| 概念                         | 说明                                                   |
| -------------------------- | ---------------------------------------------------- |
| **包（Package）**             | 一个 ZIP 格式的 `.fap` 文件，包含 `manifest.json`、平台二进制文件和资源文件 |
| **能力域（Capability Domain）** | 包内的功能分组，例如 "图片转换"、"文件操作"                             |
| **能力池（Capabilities）**   | 一个包中所有能力域的集合，即 `manifest.json` 中的 `capabilities` 对象          |
| **动作（Action）**           | 能力域内的具体操作，例如 "png2jpg"、"read"                                |
| **触桥（Bridge）协议**           | FAP 应用与宿主（FairyAction 运行时）之间的通信协议                    |
| **开发模式**                   | 零代码模式（Manifest Mapping）和 SDK 模式（fa-bridge-sdk）       |

### 两种开发模式

- **零代码模式（Manifest Mapping）**：通过 `manifest.json` 中的 `invoke` 配置直接映射 CLI 工具的参数，无需编写代码。适合已有 CLI 工具的简单包装。
- **SDK 模式**：使用 `fa-bridge-sdk` Rust crate 编写自定义逻辑，支持复杂的状态管理、持久连接和高级交互。

***

## 2. 安装与依赖

### 安装 fairy-action CLI

`fairy-action` 是 FAP 生态的工具链 CLI，提供 `fap install/sign/verify/pack` 等命令。

```bash
# 从 crates.io 安装
cargo install fairy-action

# 或从 GitHub Releases 下载预编译二进制
# 访问 https://github.com/Nicek/FairyAction/releases
```

### 添加 fa-bridge-sdk 依赖

使用 `fa-bridge-sdk` 开发 FAP 应用时，在你的 Rust 项目中添加依赖：

```bash
cargo add fa-bridge-sdk
```

或在 `Cargo.toml` 中添加：

```toml
[dependencies]
fa-bridge-sdk = "0.1"
serde_json = "1"
```

***

## 3. 快速开始

本节将引导你从零创建一个 FAP 应用。

### 2.1 创建包目录结构

```
com.example.mytool/
├── manifest.json
├── bin/
│   ├── windows-x86_64/
│   │   └── mytool.exe
│   └── linux-x86_64/
│       └── mytool
└── resources/
    └── config.toml
```

### 2.2 编写 manifest.json

创建 `com.example.mytool/manifest.json`：

```json
{
    "format_version": 1,
    "package": "com.example.mytool",
    "name": "我的工具",
    "version": "1.0.0",
    "description": "一个示例 FAP 包",
    "mode": "manifest",
    "lifecycle": "oneshot",
    "platforms": ["windows-x86_64", "linux-x86_64"],
    "entry": {
        "windows-x86_64": "bin/windows-x86_64/mytool.exe",
        "linux-x86_64": "bin/linux-x86_64/mytool"
    },
    "capabilities": {
        "文件操作": [
            {
                "名称": "读取文件",
                "参数": {
                    "path": {
                        "类型": "string",
                        "必填": true,
                        "描述": "要读取的文件路径"
                    }
                },
                "invoke": {
                    "args": ["{{来源路径的父目录}}/bin/windows-x86_64/mytool.exe", "read", "{{path}}"],
                    "timeout": 30,
                    "output": {
                        "source": "stdout",
                        "parser": "raw"
                    }
                }
            }
        ]
    },
    "permissions": ["filesystem.read"]
}
```

### 2.3 打包

```bash
fairy-action fap pack --package ./com.example.mytool/ --output ./dist/
```

这将在 `dist/` 目录下生成 `我的工具.fap` 文件。

### 2.4 安装

```bash
fairy-action fap install ./dist/我的工具.fap
```

### 2.5 验证安装

```bash
fairy-action fap list
```

输出示例：

```
com.example.mytool 1.0.0 (我的工具)
```

***

## 3. 触桥协议详解

触桥协议是 FAP 应用与宿主（FairyAction 运行时）之间的通信协议，用于传递调用请求、响应结果和进度通知。

### 3.1 消息格式

每条触桥消息遵循以下 URI 风格格式：

```
bridge://<type>\x1F<module>\x1F<channel>\x1F<action>#<JSON>
```

各字段说明：

| 字段          | 说明                                                      |
| ----------- | ------------------------------------------------------- |
| `<type>`    | 消息类型：`hello`、`call`、`ok`、`error`、`progress`、`configure` |
| `<module>`  | 包名（如 `com.ffmpeg.fap`）                                  |
| `<channel>` | 能力域名称（如 `图片转换`）                                         |
| `<action>`  | 动作名称（如 `png2jpg`）                                       |
| `<JSON>`    | JSON 格式的载荷数据                                            |
| `\x1F`      | Unit Separator（ASCII 0x1F），分隔各字段                        |
| `#`         | 分隔头部和载荷                                                 |

### 3.2 帧格式

消息在传输时使用长度前缀帧格式：

```
<length> <message>\n
```

- `<length>`：消息体的字节长度（十进制），与消息体之间用空格分隔
- `<message>`：实际的触桥消息字符串
- `\n`：换行符作为帧结束标记

示例：消息 `bridge://call\x1Fcom.ffmpeg.fap\x1F图片转换\x1Fpng2jpg#{"质量":30}` 的完整帧为：

```
58 bridge://call\x1Fcom.ffmpeg.fap\x1F图片转换\x1Fpng2jpg#{"质量":30}\n
```

### 3.3 消息类型

| 类型          | 方向     | 格式                                                                  | 说明      |
| ----------- | ------ | ------------------------------------------------------------------- | ------- |
| `hello`     | 双向     | `bridge://hello\x1F<module>#`                                       | 握手和能力查询 |
| `call`      | 宿主→FAP | `bridge://call\x1F<module>\x1F<channel>\x1F<action>#<params>`       | 调用动作    |
| `ok`        | FAP→宿主 | `bridge://ok\x1F<module>\x1F<channel>\x1F<action>#<result>`         | 成功响应    |
| `error`     | FAP→宿主 | `bridge://error\x1F<module>\x1F<channel>\x1F<action>#<error>`       | 错误响应    |
| `progress`  | FAP→宿主 | `bridge://progress\x1F<module>\x1F<channel>\x1F<action>#<progress>` | 进度通知    |
| `configure` | 宿主→FAP | `bridge://configure#<config>`                                       | 配置更新    |

#### hello 消息

用于握手和能力查询。`hello` 和 `configure` 类型只解析 `module` 字段，不解析 `channel` 和 `action`。

```
bridge://hello\x1Fcom.ffmpeg.fap#
```

#### call 消息

宿主向 FAP 应用发送调用请求：

```
bridge://call\x1Fcom.ffmpeg.fap\x1F图片转换\x1Fpng2jpg#{"质量":30}
```

#### ok 消息

FAP 应用返回成功结果：

```
bridge://ok\x1Fcom.ffmpeg.fap\x1F图片转换\x1Fpng2jpg#{"结果":"成功"}
```

#### error 消息

FAP 应用返回错误信息：

```
bridge://error\x1Fcom.ffmpeg.fap\x1F图片转换\x1Fpng2jpg#{"错误码":"HANDLER_ERROR","错误信息":"文件不存在"}
```

#### configure 消息

宿主向 FAP 应用发送配置更新，无 `module` 字段：

```
bridge://configure#{"fap.install_dir":"D:\\MyApp"}
```

### 3.4 短格式

如果 `type` 字段不是已知的消息类型（`hello`/`call`/`ok`/`error`/`progress`/`configure`），则整个第一个字段被视为 `module`，消息被解析为 `call` 类型。

例如：

```
bridge://com.ffmpeg.fap\x1F图片转换\x1Fpng2jpg#{"质量":30}
```

等价于显式的：

```
bridge://call\x1Fcom.ffmpeg.fap\x1F图片转换\x1Fpng2jpg#{"质量":30}
```

***

## 5. manifest.json 规范

`manifest.json` 是 FAP 包的核心配置文件，定义了包的元数据、能力声明和调用方式。

### 4.1 完整示例

```json
{
    "format_version": 1,
    "package": "com.example.mytool",
    "name": "我的工具",
    "version": "1.0.0",
    "description": "工具描述",
    "mode": "manifest",
    "lifecycle": "oneshot",
    "platforms": ["windows-x86_64", "linux-x86_64", "macos-arm64"],
    "entry": {
        "windows-x86_64": "bin/mytool.exe",
        "linux-x86_64": "bin/mytool",
        "macos-arm64": "bin/mytool"
    },
    "capabilities": {
        "图片转换": [
            {
                "名称": "png2jpg",
                "参数": {
                    "输入": {
                        "类型": "string",
                        "必填": true,
                        "描述": "输入文件路径"
                    },
                    "质量": {
                        "类型": "integer",
                        "默认": 85,
                        "描述": "输出质量 (1-100)"
                    }
                },
                "invoke": {
                    "args": ["{{来源路径的父目录}}/bin/mytool.exe", "-i", "{{输入}}", "-q", "{{质量}}"],
                    "env": {
                        "LANG": "en_US.UTF-8"
                    },
                    "exit_code": {
                        "0": "success"
                    },
                    "output": {
                        "source": "stdout",
                        "parser": "json"
                    },
                    "timeout": 60
                }
            }
        ]
    },
    "permissions": ["filesystem.read", "filesystem.write"],
    "signature": {
        "algorithm": "Ed25519",
        "value": "<base64 signature>",
        "public_key": "<base64 public key>"
    }
}
```

### 4.2 字段说明

#### 顶层字段

| 字段               | 类型        | 必填 | 说明                                                                                             |
| ---------------- | --------- | -- | ---------------------------------------------------------------------------------------------- |
| `format_version` | integer   | 是  | 格式版本号，当前为 `1`                                                                                  |
| `package`        | string    | 是  | 反向域名格式的包标识符（如 `com.example.mytool`），不可为空                                                       |
| `name`           | string    | 是  | 人类可读的包名                                                                                        |
| `version`        | string    | 是  | 语义化版本号，格式为 `x.y.z`                                                                             |
| `description`    | string    | 否  | 包的描述信息                                                                                         |
| `mode`           | string    | 是  | 开发模式：`"manifest"`（零代码）或 `"sdk"`（需 fa-bridge-sdk）                                               |
| `lifecycle`      | string    | 否  | 生命周期：`"oneshot"`、`"persistent"` 或 `"both"`                                                     |
| `platforms`      | string\[] | 是  | 支持的平台列表，不可为空。格式为 `<os>-<arch>`，如 `windows-x86_64`、`linux-x86_64`、`macos-arm64`、`macos-aarch64` |
| `entry`          | object    | 是  | 各平台的入口文件路径（相对于包根目录），至少包含一个平台。所有 entry 中的平台必须出现在 platforms 列表中                                  |
| `capabilities`   | object    | 条件 | 能力域映射，mode 为 `manifest` 时不可为空                                                                  |
| `permissions`    | string\[] | 否  | 权限声明数组，默认为空                                                                                    |
| `signature`      | object    | 否  | 签名信息，由 `fap sign` 命令自动填写                                                                       |

#### capabilities 字段

`capabilities` 是一个对象，键为能力域名称（如 `"图片转换"`），值为该域下的动作数组。

```json
{
    "capabilities": {
        "<能力域名称>": [
            { "<动作定义>" },
            { "<动作定义>" }
        ]
    }
}
```

#### 动作定义

每个动作包含以下字段：

| 字段       | 类型     | 必填 | 说明                  |
| -------- | ------ | -- | ------------------- |
| `名称`     | string | 是  | 动作名称                |
| `参数`     | object | 否  | 参数定义，键为参数名，值为参数属性对象 |
| `invoke` | object | 否  | 调用配置（零代码模式必须）       |

#### 参数定义

参数使用对象形式定义（键为参数名）：

```json
{
    "path": {
        "类型": "string",
        "必填": true,
        "描述": "文件路径"
    },
    "质量": {
        "类型": "integer",
        "默认": 85,
        "描述": "输出质量"
    }
}
```

每个参数的属性：

| 属性   | 类型      | 必填 | 说明                                         |
| ---- | ------- | -- | ------------------------------------------ |
| `类型` | string  | 是  | 参数类型：`string`、`integer`、`number`、`boolean` |
| `必填` | boolean | 否  | 是否必填，默认 `false`                            |
| `描述` | string  | 否  | 参数描述                                       |
| `默认` | any     | 否  | 默认值                                        |

#### invoke 配置

`invoke` 定义了零代码模式下如何调用底层二进制：

| 字段          | 类型        | 必填 | 说明                      |
| ----------- | --------- | -- | ----------------------- |
| `args`      | string\[] | 是  | 参数模板数组，支持 `{{变量名}}` 占位符 |
| `env`       | object    | 否  | 环境变量映射                  |
| `exit_code` | object    | 否  | 退出码映射                   |
| `output`    | object    | 否  | 输出配置                    |
| `timeout`   | integer   | 否  | 超时时间（秒）                 |

#### output 配置

| 字段        | 类型     | 默认值        | 说明                           |
| --------- | ------ | ---------- | ---------------------------- |
| `source`  | string | `"stdout"` | 输出来源：`"stdout"` 或 `"stderr"` |
| `parser`  | string | —          | 解析器名称（见第 7 章）                |
| `pattern` | string | 否          | 正则表达式（`regex` 解析器必须）         |

#### signature 字段

由 `fap sign` 命令自动生成并写入：

| 字段           | 类型     | 说明                   |
| ------------ | ------ | -------------------- |
| `algorithm`  | string | 签名算法，当前为 `"Ed25519"` |
| `value`      | string | Base64 编码的签名字节       |
| `public_key` | string | Base64 编码的公钥         |

### 4.3 验证规则

`manifest.json` 在安装时会进行以下验证：

1. `package` 不可为空字符串
2. `platforms` 不可为空数组
3. `entry` 至少包含一个平台
4. `entry` 中的每个平台必须出现在 `platforms` 列表中
5. `mode` 为 `manifest` 时 `capabilities` 不可为空
6. `permissions` 中的每个权限必须是合法的（见第 9 章）

***

## 5. 开发模式

### 5.1 零代码模式（Manifest Mapping）

零代码模式通过 `manifest.json` 中的 `invoke` 配置直接映射 CLI 工具的参数，无需编写任何代码。

#### invoke 模板语法

`invoke.args` 数组中的每个字符串都是一个模板，支持以下占位符：

| 占位符            | 说明                                        |
| -------------- | ----------------------------------------- |
| `{{参数名}}`      | 替换为调用时传入的参数值                              |
| `{{临时目录}}`     | 替换为系统临时目录路径                               |
| `{{包目录}}`      | 替换为已安装的 FAP 包目录路径                         |
| `{{宿主数据目录}}`   | 替换为宿主数据目录路径（需通过 `--fap-host-data-dir` 配置） |
| `{{来源路径的父目录}}` | 替换为包目录的父目录路径                              |

> **⚠️ 重要：`args`** **是传给二进制的参数，不是命令本身**
>
> `entry` 字段已经指定了要执行的可执行文件路径，`invoke.args` 只需要放**传给该二进制的命令行参数**。
>
> 执行流程：`<entry 二进制> <args[0]> <args[1]> ...`
>
> ```jsonc
> // ❌ 错误：不要在 args 中重复指定二进制路径
> "invoke": {
>     "args": ["{{来源路径的父目录}}/bin/ffmpeg.exe", "-i", "{{输入}}"]
> }
> // 这会导致实际执行：ffmpeg.exe {{来源路径的父目录}}/bin/ffmpeg.exe -i input — 二进制被传了两次！
>
> // ✅ 正确：args 只放参数
> "invoke": {
>     "args": ["-i", "{{输入}}", "-q:v", "{{质量}}", "{{输出}}"]
> }
> // 实际执行：ffmpeg.exe -i input -q:v 90 output.jpg
> ```

#### 参数类型映射

| 参数 JSON 类型       | 渲染结果                 |
| ---------------- | -------------------- |
| string           | 原始字符串                |
| integer / number | 转为字符串                |
| boolean          | `"true"` 或 `"false"` |
| array            | 展开为多个独立参数            |

#### 完整示例：FFmpeg 封装

```json
{
    "名称": "png2jpg",
    "参数": {
        "输入": {
            "类型": "string",
            "必填": true,
            "描述": "输入 PNG 文件路径"
        },
        "输出": {
            "类型": "string",
            "必填": true,
            "描述": "输出 JPG 文件路径"
        },
        "质量": {
            "类型": "integer",
            "默认": 85,
            "描述": "输出质量 (1-100)"
        }
    },
    "invoke": {
        "args": ["{{来源路径的父目录}}/bin/ffmpeg.exe", "-i", "{{输入}}", "-q:v", "{{质量}}", "{{输出}}"],
        "output": {
            "source": "stdout",
            "parser": "last_line"
        },
        "timeout": 120
    }
}
```

当调用 `png2jpg` 并传入 `{"输入": "a.png", "输出": "b.jpg", "质量": 90}` 时，系统会执行：

```
/opt/fap/com.ffmpeg.fap/bin/ffmpeg.exe -i a.png -q:v 90 b.jpg
```

#### 数组参数展开示例

```json
{
    "名称": "批量处理",
    "参数": {
        "文件列表": {
            "类型": "array",
            "必填": true
        }
    },
    "invoke": {
        "args": ["{{来源路径的父目录}}/bin/tool.exe", "process", "{{文件列表}}"]
    }
}
```

传入 `{"文件列表": ["a.txt", "b.txt", "c.txt"]}` 时，渲染结果为：

```
["/path/bin/tool.exe", "process", "a.txt", "b.txt", "c.txt"]
```

### 5.2 SDK 模式

SDK 模式使用 `fa-bridge-sdk` crate 编写自定义 Rust 程序，适用于需要复杂逻辑、状态管理或持久连接的场景。

`fa-bridge-sdk` 是一个独立的 crate，仅依赖 `serde`、`serde_json`、`thiserror` 和 `anyhow`，可发布到 [crates.io](https://crates.io)。

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
fa-bridge-sdk = "0.1"
```

SDK 模式支持两种运行方式：

- **Oneshot 模式**：每次调用启动新进程，从命令行参数解析调用
- **Persistent 模式**：长驻进程，通过 stdin/stdout 进行消息通信

***

## 6. fa-bridge-sdk API 参考

### 6.1 完整示例

```rust
use fa_bridge_sdk::{App, Domain, Action, Param, Lifecycle, ActionContext};
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
                        let quality = params["质量"].as_u64().unwrap_or(85);
                        Ok(Value::String(format!(
                            "converted {} with quality {}",
                            input, quality
                        )))
                    })
                    .param(Param::string("输入").required().desc("输入文件路径"))
                    .param(Param::int("质量").default_val(Value::from(85)))
                )
        );

    app.run();
}
```

### 6.2 类型说明

#### `App` — 应用构建器

应用构建器，用于定义和运行 FAP 应用。

```rust
// 创建空应用
let app = App::new();

// 设置应用名
app.name("my-tool")

// 设置版本号（默认 "1.0.0"）
app.version("2.0.0")

// 设置生命周期模式（默认 Both）
app.lifecycle(Lifecycle::Oneshot)

// 添加能力域
app.domain(Domain::new("图片转换").action(/* ... */))

// 启动应用
app.run()
```

`run()` 方法根据命令行参数自动选择运行模式：

| 启动参数                           | 行为                            |
| ------------------------------ | ----------------------------- |
| `--capabilities`               | 输出 JSON 格式的能力池描述并退出           |
| `--serve`                      | 进入 persistent 模式，从 stdin 读取消息 |
| `<action> [--param value ...]` | oneshot 模式，执行指定动作             |

#### `Domain` — 能力域

```rust
// 创建能力域
Domain::new("图片转换")

// 设置描述
.description("图片格式转换功能")

// 添加动作
.action(Action::new(/* ... */))
```

#### `Action` — 动作定义

```rust
// 创建动作，handler 签名为 Fn(Value, &ActionContext) -> anyhow::Result<Value>
Action::new("png2jpg", |params, ctx| {
    // params: serde_json::Value，调用时传入的参数
    // ctx: &ActionContext，执行上下文
    Ok(serde_json::json!({"result": "ok"}))
})

// 设置描述
.description("将 PNG 转换为 JPG")

// 添加参数
.param(Param::string("输入").required())
```

#### `Param` — 参数定义

参数通过类型化的构造函数创建：

```rust
// 字符串参数
Param::string("文件路径").required().desc("输入文件路径")

// 整数参数
Param::int("质量").default_val(serde_json::json!(85))

// 布尔参数
Param::bool_val("详细输出").default_val(serde_json::json!(false))

// 数组参数
Param::array("文件列表")
```

每个 `Param` 方法说明：

| 方法                      | 说明        |
| ----------------------- | --------- |
| `Param::string(name)`   | 创建字符串类型参数 |
| `Param::int(name)`      | 创建整数类型参数  |
| `Param::bool_val(name)` | 创建布尔类型参数  |
| `Param::array(name)`    | 创建数组类型参数  |
| `.required()`           | 标记为必填     |
| `.desc(text)`           | 设置参数描述    |
| `.default_val(value)`   | 设置默认值     |

#### `Lifecycle` — 生命周期枚举

```rust
pub enum Lifecycle {
    Oneshot,      // 每次调用启动新进程
    Persistent,   // 长驻进程，通过 stdin/stdout 通信
    Both,         // 两种模式都支持（默认）
}
```

#### `ActionContext` — 动作执行上下文

```rust
pub struct ActionContext {
    pub domain: String,    // 当前能力域名称
    pub action: String,    // 当前动作名称
    pub mode: RunMode,     // 当前运行模式
}

// 发送进度通知（仅 persistent 模式有效）
ctx.progress(50, "处理中...")?;
```

`progress` 方法参数：

| 参数        | 类型     | 说明            |
| --------- | ------ | ------------- |
| `percent` | `u32`  | 进度百分比 (0-100) |
| `status`  | `&str` | 状态描述文本        |

在 oneshot 模式下调用 `progress()` 不会产生任何输出。

### 6.3 Oneshot 模式详解

命令行调用格式：

```bash
my-tool png2jpg --输入 a.png --质量 90
```

参数解析规则：

- `--key value` → 字符串参数（值会自动推断类型）
- 整数字符串（如 `90`）→ 解析为 `i64`
- `"true"` / `"false"` → 解析为布尔值
- `--flag`（无后续值）→ 解析为 `true`

成功时输出 JSON 到 stdout：

```json
{
  "result": "converted a.png with quality 90"
}
```

失败时输出错误到 stderr 并以退出码 1 退出：

```json
{
  "error": "文件不存在: a.png"
}
```

`--capabilities` 参数输出能力池描述：

```bash
my-tool --capabilities
```

输出示例：

```json
{
  "能力域": [
    {
      "名称": "图片转换",
      "动作": [
        {
          "名称": "png2jpg",
          "参数": {
            "输入": {
              "类型": "字符串",
              "必填": true,
              "描述": "输入文件路径"
            },
            "质量": {
              "类型": "整数",
              "默认": 85
            }
          }
        }
      ]
    }
  ]
}
```

### 6.4 Persistent 模式详解

启动命令：

```bash
my-tool --serve
```

#### 握手

启动时输出握手消息到 stdout：

```
hello\x1Fmy-tool\x1F1.0.0\x1F{"能力域":[...]}\n
```

#### 消息循环

进入消息循环后，从 stdin 逐行读取消息。消息格式为 `\x1F` 分隔的文本行：

```
call\x1F图片转换\x1Fpng2jpg\x1F{"输入":"a.png","质量":90}\n
```

成功时响应：

```
ok\x1F图片转换\x1Fpng2jpg\x1F{"result":"ok"}\n
```

失败时响应：

```
error\x1F图片转换\x1Fpng2jpg\x1F{"错误码":"HANDLER_ERROR","错误信息":"文件不存在"}\n
```

#### 关闭

收到 `shutdown` 消息时输出 `bye\n` 并退出：

```
shutdown → bye\n
```

#### 错误处理

- 找不到动作：`{"错误码":"NOT_FOUND","错误信息":"action not found: 域名 / 动作名"}`
- 未知消息类型：`{"错误码":"UNKNOWN_MESSAGE","错误信息":"unknown message type: xxx"}`

***

## 7. 内置解析器

解析器用于将 CLI 工具的原始输出转换为结构化 JSON。在 `invoke.output.parser` 中指定解析器名称。

| 解析器               | 说明                     | 输入示例                                    | 输出示例                                        |
| ----------------- | ---------------------- | --------------------------------------- | ------------------------------------------- |
| `raw`             | 将原始文本包装为 JSON 对象       | `hello world`                           | `{"output": "hello world"}`                 |
| `json`            | 直接解析 JSON 输出           | `{"status":"ok"}`                       | `{"status":"ok"}`                           |
| `last_line`       | 取最后一行文本                | `line1\nline2\nresult`                  | `{"output": "result"}`                      |
| `lines`           | 按行分割为数组                | `a\nb\nc`                               | `{"lines": ["a","b","c"]}`                  |
| `csv`             | CSV 解析，每行按逗号分割         | `name,age\nTom,25`                      | `{"rows": [["name","age"],["Tom","25"]]}`   |
| `ffmpeg_progress` | 解析 ffmpeg 进度输出         | `frame=100 fps=30 time=00:01:23.45 ...` | `{"time":"00:01:23.45","进度":"00:01:23.45"}` |
| `regex`           | 正则匹配（需配合 `pattern` 参数） | 任意输入                                    | 根据正则匹配结果生成                                  |

### 解析器使用示例

#### raw 解析器

适用于任何文本输出，将完整输出包装为 `{"output": "..."}`：

```json
{
    "output": {
        "source": "stdout",
        "parser": "raw"
    }
}
```

#### json 解析器

直接解析 JSON 输出，要求输出为合法 JSON：

```json
{
    "output": {
        "source": "stdout",
        "parser": "json"
    }
}
```

#### regex 解析器

使用 `pattern` 字段指定正则表达式。匹配结果以 `full_match` 和 `group_1`、`group_2`... 形式返回：

```json
{
    "output": {
        "source": "stdout",
        "parser": "regex",
        "pattern": "(\\d+) files processed"
    }
}
```

输入 `42 files processed` 输出：

```json
{
    "full_match": "42 files processed",
    "group_1": "42"
}
```

未匹配时返回空对象 `{}`。

#### ffmpeg\_progress 解析器

从 ffmpeg 的 stderr 输出中提取 `time=` 字段：

```json
{
    "output": {
        "source": "stderr",
        "parser": "ffmpeg_progress"
    }
}
```

***

## 9. 打包工具链

### 8.1 完整打包流程

```bash
# 1. 生成密钥对
fairy-action fap keygen --output ./keys/

# 2. 签名 FAP 包
fairy-action fap sign --key ./keys/fap_private.key --package ./com.example.mytool/

# 3. 验证签名
fairy-action fap verify --package ./com.example.mytool/

# 4. 打包为 .fap 文件
fairy-action fap pack --package ./com.example.mytool/ --output ./dist/
```

### 8.2 .fap 文件格式

`.fap` 文件本质上是 ZIP 格式（使用 Deflate 压缩），包含包目录下的所有文件。打包时会自动排除以 `.` 开头的文件和目录（隐藏文件）。

打包后的 ZIP 内部路径使用 `/` 分隔符（即使在 Windows 上），例如：

```
manifest.json
bin/windows-x86_64/mytool.exe
bin/linux-x86_64/mytool
resources/config.toml
signature.sig
```

### 8.3 签名流程详解

签名算法为 **Ed25519**，摘要计算流程如下：

1. **收集文件**：遍历包目录下的所有文件（排除 `signature.sig` 和 `manifest.json`）
2. **路径排序**：按相对路径字典序排序
3. **逐文件哈希**：对每个文件内容计算 SHA-256 摘要，格式化为十六进制字符串
4. **拼接哈希**：将所有文件的 `<相对路径>:<十六进制哈希>\n` 拼接
5. **最终摘要**：对拼接字符串计算 SHA-256 得到 32 字节的最终摘要
6. **签名**：使用 Ed25519 私钥对最终摘要进行签名

生成的文件：

- `signature.sig`：签名文件（64 字节原始签名数据）
- `manifest.json` 中更新 `signature` 字段

密钥文件：

- `fap_private.key`：64 字节（32 字节种子 + 32 字节公钥）
- `fap_public.key`：32 字节公钥

***

## 9. 权限系统

FAP 包需要在 `manifest.json` 中声明所需的权限。未声明的权限项会导致 manifest 验证失败。

### 合法权限列表

| 权限                 | 说明     |
| ------------------ | ------ |
| `filesystem.read`  | 读取文件系统 |
| `filesystem.write` | 写入文件系统 |
| `network.outbound` | 出站网络访问 |
| `process.spawn`    | 启动子进程  |
| `clipboard.read`   | 读取剪贴板  |
| `clipboard.write`  | 写入剪贴板  |

### 使用方式

声明所需权限：

```json
{
    "permissions": ["filesystem.read", "filesystem.write"]
}
```

不声明 `permissions` 或声明空数组表示不需要额外权限：

```json
{
    "permissions": []
}
```

声明非法权限项会导致验证失败：

```json
{
    "permissions": ["filesystem.read", "evil.permission"]
}
```

错误信息：`unknown permission: evil.permission`

***

## 10. 签名与安全

### 10.1 签名流程

1. `fairy-action fap keygen` 生成 Ed25519 密钥对
2. `fairy-action fap sign` 计算包内文件摘要并用私钥签名
3. `fairy-action fap verify` 用公钥验证签名完整性
4. `fairy-action fap install` 安装时自动验证签名（可用 `--skip-verify` 跳过）

### 10.2 安装时签名验证行为

| 场景       | 行为                               |
| -------- | -------------------------------- |
| 已签名且验证通过 | 正常安装，显示 `Signature verified: OK` |
| 已签名但验证失败 | **回滚安装**（删除已解压的文件），报错终止          |
| 未签名      | 显示警告 `包未签名，无法验证完整性`，继续安装         |

### 10.3 版本变更

如果覆盖安装（已存在同名包），系统会检测版本变更并提示：

```
Installed: 我的工具 v1.0.0
Version changed: 0.9.0 -> 1.0.0
```

### 10.4 安全建议

- 私钥文件 (`fap_private.key`) 应妥善保管，不要泄露或提交到版本控制
- 发布前务必签名，确保包的完整性
- 使用 `fairy-action fap verify` 在分发前验证签名

***

## 11. CLI 命令参考

所有 FAP 相关命令通过 `fairy-action fap` 子命令访问。

### fairy-action fap install

安装 FAP 包。

```bash
fairy-action fap install <path> [--skip-verify]
```

| 参数              | 说明            |
| --------------- | ------------- |
| `<path>`        | `.fap` 文件路径   |
| `--skip-verify` | 跳过签名验证（默认会验证） |

示例：

```bash
fairy-action fap install ./dist/我的工具.fap
fairy-action fap install ./dist/我的工具.fap --skip-verify
```

### fairy-action fap uninstall

卸载 FAP 包。

```bash
fairy-action fap uninstall <package>
```

| 参数          | 说明                           |
| ----------- | ---------------------------- |
| `<package>` | 包标识符（如 `com.example.mytool`） |

示例：

```bash
fairy-action fap uninstall com.example.mytool
```

### fairy-action fap list

列出已安装的 FAP 包。

```bash
fairy-action fap list
```

输出示例：

```
com.example.mytool 1.0.0 (我的工具)
com.ffmpeg.fap 4.2.1 (FFmpeg 工具集)
```

### fairy-action fap inspect

查看已安装 FAP 包的详细信息。

```bash
fairy-action fap inspect <package>
```

| 参数          | 说明   |
| ----------- | ---- |
| `<package>` | 包标识符 |

输出完整的 `manifest.json` 内容（格式化 JSON）。

### fairy-action fap run

直接运行 FAP 包中的动作，用于调试。

```bash
fairy-action fap run <package> <capability> <action> [params] [options]
```

| 参数                          | 说明                 |
| --------------------------- | ------------------ |
| `<package>`                 | 包标识符               |
| `<capability>`              | 能力域名称              |
| `<action>`                  | 动作名称               |
| `[params]`                  | JSON 格式的参数，默认 `{}` |
| `--fap-install-dir <dir>`   | 覆盖安装目录             |
| `--fap-temp-dir <dir>`      | 覆盖临时目录             |
| `--fap-host-data-dir <dir>` | 设置宿主数据目录           |

示例：

```bash
fairy-action fap run com.example.mytool "文件操作" "读取文件" '{"path":"/tmp/test.txt"}'
```

### fairy-action fap keygen

生成 Ed25519 密钥对。

```bash
fairy-action fap keygen [--output <dir>]
```

| 参数               | 说明           |
| ---------------- | ------------ |
| `--output <dir>` | 输出目录，默认为当前目录 |

生成的文件：

- `<dir>/fap_private.key` — 64 字节私钥
- `<dir>/fap_public.key` — 32 字节公钥

示例：

```bash
fairy-action fap keygen --output ./keys/
```

### fairy-action fap sign

签名 FAP 包。

```bash
fairy-action fap sign --key <path> --package <dir>
```

| 参数                | 说明                        |
| ----------------- | ------------------------- |
| `--key <path>`    | 私钥文件路径（`fap_private.key`） |
| `--package <dir>` | FAP 包目录路径                 |

示例：

```bash
fairy-action fap sign --key ./keys/fap_private.key --package ./com.example.mytool/
```

签名后会：

1. 在包目录下生成 `signature.sig` 文件
2. 更新 `manifest.json` 中的 `signature` 字段

### fairy-action fap verify

验证 FAP 包签名。

```bash
fairy-action fap verify --package <dir>
```

| 参数                | 说明        |
| ----------------- | --------- |
| `--package <dir>` | FAP 包目录路径 |

示例：

```bash
fairy-action fap verify --package ./com.example.mytool/
```

### fairy-action fap pack

打包 FAP 包目录为 `.fap` 文件。

```bash
fairy-action fap pack --package <dir> [--output <path>] [--verify]
```

| 参数                | 说明                  |
| ----------------- | ------------------- |
| `--package <dir>` | FAP 包目录路径           |
| `--output <path>` | 输出路径（目录或文件），默认为当前目录 |
| `--verify`        | 打包前验证签名（默认 `true`）  |

示例：

```bash
fairy-action fap pack --package ./com.example.mytool/ --output ./dist/
```

如果 `--output` 指定为目录或无扩展名路径，输出文件名为 `<包名>.fap`。

### fairy-action bridge

触桥协议交互模式，从 stdin 读取触桥帧消息，处理后返回响应。

```bash
fairy-action bridge [--fap-install-dir <dir>] [--fap-temp-dir <dir>] [--fap-host-data-dir <dir>]
```

| 参数                          | 说明          |
| --------------------------- | ----------- |
| `--fap-install-dir <dir>`   | 覆盖 FAP 安装目录 |
| `--fap-temp-dir <dir>`      | 覆盖临时文件目录    |
| `--fap-host-data-dir <dir>` | 设置宿主数据目录    |

***

## 13. 最佳实践与 FAQ

### 最佳实践

1. **包名规范**：使用反向域名格式（如 `com.example.mytool`），确保全局唯一性
2. **版本号规范**：遵循语义化版本（`x.y.z`），明确标记破坏性变更
3. **动作描述**：为每个动作提供清晰的中文描述和参数说明，便于 AI 理解
4. **最小权限**：只声明所需的最小权限集合，不申请不必要的权限
5. **签名发布**：正式发布前务必签名，保障包的完整性和可信度
6. **选择合适的解析器**：根据 CLI 工具的输出格式选择合适的解析器（见第 7 章）
7. **合理设置超时**：为耗时操作设置合理的 `timeout` 值，避免无限等待
8. **隐藏文件排除**：打包时以 `.` 开头的文件和目录会被自动排除，可用于存放构建脚本等

### FAQ

**Q: 零代码模式和 SDK 模式如何选择？**

A: 如果你的工具已经是 CLI 程序，只需简单映射参数，使用零代码模式（`mode: "manifest"`）。如果需要复杂逻辑（如状态管理、条件分支、数据处理），或需要持久连接（如流式处理），使用 SDK 模式（`mode: "sdk"`）。

**Q: persistent 模式的进程什么时候会被回收？**

A: persistent 模式的进程通过 stdin/stdout 通信。当 stdin 关闭（EOF）或收到 `shutdown` 消息时，进程应输出 `bye` 并正常退出。宿主系统会根据需要管理进程的生命周期。

**Q: 如何调试 FAP 应用？**

A: 使用以下方法：

- `fairy-action fap run` 命令可以直接运行动作并查看输出
- SDK 模式的 oneshot 方式可以直接从命令行运行，方便调试
- 检查 stderr 输出获取错误信息
- 使用 `fairy-action fap inspect` 查看已安装包的配置

**Q: 支持哪些平台？**

A: 平台标识格式为 `<os>-<arch>`，当前支持检测的平台包括：

- `windows-x86_64`
- `linux-x86_64`
- `linux-aarch64`
- `linux-arm`
- `macos-x86_64`
- `macos-arm64`（也标识为 `macos-aarch64`）

你可以在 `platforms` 和 `entry` 中声明多个平台，系统会在运行时自动检测当前平台并选择对应的入口二进制。
