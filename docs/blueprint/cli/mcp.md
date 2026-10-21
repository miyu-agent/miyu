## `miyu mcp`

### 是什么

`miyu mcp login <服务>`：登录一个只认浏览器授权的网址 MCP 服务（施工 X-42 上，`mcp.md`「怎么走」第八条）。只是协议的客户端：调 `mcp.login`，令牌由核心换、存，命令行一个字也不经手。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | `mcp` 子命令交给 `miyu_cli::mcp`，帮助页照界面语言 |
| `crates/miyu-cli/src/mcp.rs` | 连上核心（没在跑的拉起）、`mcp.login`、印地址、打开浏览器、照 `mcp.list` 等连上 |
| `crates/miyu-cli/src/language/mcp.rs` | 给人看的几句（中英） |
| `crates/miyu-cli/src/help/{zh,en}/mcp.txt` | 帮助页 |

### 对外的样子

```text
miyu mcp login <服务>
```

1. 连上核心（没在跑的拉起），发 `mcp.login {service}`。核心拒了的照它的原话印在标准错误上，退出码 1。
2. 标准输出先印一句「在浏览器里打开这个地址登录 <服务>：」，下一行是授权页的地址（只有地址，不带 `code_verifier` 这一类）。
3. 试着打开浏览器：Linux 这类 `xdg-open`，macOS `open`，Windows `rundll32 url.dll,FileProtocolHandler`（`cmd /c start` 会把网址里的 `&` 当分隔符）。打不开的在标准错误上说一句，人自己复制地址。
4. 等连上：个人设置里的服务每秒照 `mcp.list` 看一次，`running` 了印「登录好了」、退出码 0；10 分钟还没连上的说一句、退出码 1。包里带的服务不在 `mcp.list` 里：印一句「登录好以后服务会自己连上」，退出码 0。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-cli/src/mcp/tests.rs` | 照 `mcp.list` 认状态，包里带的不在里面；网址带着 `&` 也整个交给打开的程序 |
