# fa-bridge-sdk

FairyAction Bridge SDK — 用于开发 FAP (FairyAction Package) 应用的 Rust SDK。

## 概述

`fa-bridge-sdk` 是 [FairyAction](https://github.com/) FAP 生态的开发者 SDK。它帮助开发者将原生工具包装为 AI Agent 可调用的能力单元，通过触桥（Bridge）协议与 FairyAction 运行时通信。

**特点**：
- 完全独立，不依赖 FairyAction 内部 crate，可独立发布到 crates.io
- 支持两种生命周期模式：Oneshot（CLI 调用）和 Persistent（长驻进程）
- Builder API 风格，简洁易用
- 自动处理协议消息的序列化/反序列化

## 快速开始

### 添加依赖

```toml
[dependencies]
fa-bridge-sdk = "0.1"
serde_json = "1"
```

### 最简示例

```rust
use fa_bridge_sdk::{App, Domain, Action, Lifecycle};
use serde_json::Value;

fn main() {
    let app = App::new()
        .name("my-tool")
        .version("1.0.0")
        .lifecycle(Lifecycle::Oneshot)
        .domain(
            Domain::new("问候")
                .action(Action::new("打招呼", |_ctx, params| {
                    let name = params["名字"].as_str().unwrap_or("世界");
                    Ok(Value::String(format!("你好，{}！", name)))
                }))
        );

    app.run();
}
```

运行：`my-tool 打招呼 --名字 "FairyAction"`

输出：`"你好，FairyAction！"`

## 两种模式

### Oneshot 模式

每次调用启动新进程，从命令行参数读取输入，输出 JSON 到 stdout。适合无状态的 CLI 工具包装。

```rust
let app = App::new()
    .name("image-tool")
    .version("1.0.0")
    .lifecycle(Lifecycle::Oneshot)
    .domain(
        Domain::new("图片转换")
            .action(
                Action::new("png2jpg", |_ctx, params| {
                    let input = params["输入"].as_str().unwrap();
                    let quality = params["质量"].as_u64().unwrap_or(85);
                    // 执行转换逻辑...
                    Ok(serde_json::json!({"output": format!("{}.jpg", input), "quality": quality}))
                })
            )
    );
```

调用方式：`image-tool png2jpg --输入 photo.png --质量 90`

### Persistent 模式

长驻进程，通过 stdin/stdout 通信。适合需要状态管理或频繁调用的场景。

```rust
let app = App::new()
    .name("image-tool")
    .version("1.0.0")
    .lifecycle(Lifecycle::Persistent)
    .domain(
        Domain::new("图片转换")
            .action(
                Action::new("png2jpg", |ctx, params| {
                    ctx.progress(50, "正在转换...");
                    // 执行转换逻辑...
                    ctx.progress(100, "完成");
                    Ok(serde_json::json!({"status": "ok"}))
                })
            )
    );
```

启动：`image-tool --serve`

进程会通过 stdin/stdout 自动处理触桥协议消息，支持 hello 握手、call 调用、progress 进度通知、shutdown 关闭。

## API 参考

### App

应用构建器，SDK 的入口点。

```rust
App::new()                          // 创建空应用
    .name("tool-name")              // 设置应用名
    .version("1.0.0")              // 设置版本号
    .lifecycle(Lifecycle::Oneshot)  // 设置生命周期
    .domain(domain)                 // 添加能力域
    .run()                          // 启动应用
```

### Domain / Action / Param

```rust
Domain::new("能力域名称")
    .action(
        Action::new("动作名称", handler)
            .param(Param::string("参数名").required(true))
            .param(Param::int("数量").default_value(serde_json::json!(10)))
    )
```

Handler 签名：`Fn(Value, &ActionContext) -> Result<Value, Box<dyn Error>>`

### ActionContext

```rust
ctx.domain       // 当前能力域名称
ctx.action       // 当前动作名称
ctx.progress(50, "进度信息")  // 发送进度通知（仅 persistent 模式）
```

### Lifecycle

```rust
Lifecycle::Oneshot     // CLI 模式
Lifecycle::Persistent  // 长驻进程模式
Lifecycle::Both        // 两种都支持（通过 --serve 参数切换）
```

## 与 FAP 生态集成

使用 `fa-bridge-sdk` 开发的应用需要打包为 FAP 格式才能被 FairyAction 运行时发现和调用。

1. 编译你的应用为二进制
2. 创建 FAP 包目录（含 `manifest.json`，mode 设为 `"sdk"`）
3. 使用 `fairy-action fap pack` 打包
4. 使用 `fairy-action fap install` 安装

详细文档请参阅 [FAP 开发者指南](https://github.com/)。

## License

MIT
