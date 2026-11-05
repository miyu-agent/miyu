//! 包的编号不能用的名字（施工 FH-3上 起在 `miyu-store`，FH-5下 挪到这里：软件源的收录也要查，`miyu-index` 不为它拉进存储那一串）。

use super::{Code, Problem};

/// 核心自己在数据根的 `etc/`、`var/lib/`、`var/cache/`、`var/log/`、`tmp/`、`run/` 这几层用到的名字（设计 55 第三节第 3 条）：包
/// 各占一个 `<编号>/`，这些不能用作包的编号。`miyu-store` 的测试照各层的方法推一遍，和这张表对。
pub const RESERVED: [&str; 18] = [
    "private", "removed", "leftover", "models", "packages", "mcp", "index", "config", "provide",
    "scripts", "personas", "mermaid", "sandbox", "perf", "unpack", "token", "socket", "log-salt",
];

/// 编号 `id` 是保留的名字的话，那一条问题（核心读清单、装包时查，软件源收录时查）；不是的没有。
pub fn reserved_id(id: &str) -> Option<Problem> {
    RESERVED.contains(&id).then(|| Problem {
        line: None,
        code: Code::ReservedId,
        message: format!("package id {id} is reserved in the data root"),
        detail: id.to_string(),
    })
}
