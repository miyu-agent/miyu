//! 软件源（设计 `37-软件源.md`）：拿索引（施工 G-3 上，`sync`）、搜（G-3 下，`search`）、从软件源装（G-4 上，`fetch`、
//! `download`）、清缓存（G-4 下，`clean`）、升级
//! （G-5，`upgrade`）、`package.info` 查软件源（施工 IN-2，`look`）。

pub(crate) mod clean;
mod download;
pub(crate) mod feeds;
pub(crate) mod fetch;
pub(crate) mod logo;
pub(crate) mod look;
pub(crate) mod search;
pub(crate) mod sync;
pub(crate) mod upgrade;
