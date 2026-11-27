# IP-1 `miyu-ipc` 单独编得过

状态：待验收（2026-11-28）

## 为什么

2026-11-27 im 会话给主人装 rc.5，托盘钉核心 015f83b2a 编不过：`crates/miyu-ipc/src/start/spawn.rs` 用了 `tokio::select!`（CS-6 加的），`miyu-ipc` 自己的 `[dependencies]` 里 tokio 却没开 `macros`，只有 `[dev-dependencies]` 开了。核心的工作区里别的包开了这个特性，整体编、整体测时合在一起，看不出来；托盘只拿 `miyu-client`，单独编就露出来了。

## 改了什么

- `crates/miyu-ipc/Cargo.toml`：tokio 加上 `macros`。
- 门禁加一道「单独编」（`xtask/src/alone.rs`）：外面的仓库直接拿去用的几个包（`miyu-client`、`miyu-ipc`、`miyu-store`、`miyu-config`、`miyu-recall`）各自单独 `cargo check -p` 一次，特性不和别的包合。终端界面、im 拿的包多，特性本来就合在一起，不在这里逐个查。

## 怎么验

- 修之前：`cargo check -p miyu-ipc` 报两处 `cannot find select in tokio`。
- 修之后：五个包各自单独 `cargo check -p` 都过，门禁全过。
