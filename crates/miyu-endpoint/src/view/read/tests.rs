//! 翻页只读要的那一截（施工 V-41 上）的对照测试：以前的读法照抄在 `old.rs`，`script.rs` 照种子造会话日志，`same.rs` 新旧逐字
//! 比。整个会话喂投影（施工 SE-1 中下上，`view/whole.rs`）照同一个造法对照整份读的（`same::made`）。

mod old;
pub(in crate::view) mod same;
mod script;
