//! Linux 上照规格建 Landlock 规则集（`docs/blueprint/sandbox/linux.md`；施工 V-47 从助手挪进库里，助手和核心用同一份）：
//!
//! - [`confine`]：助手每条命令从头建、收紧自己（和施工 V-47 以前一样）；
//! - [`confine_cached`]：助手照核心交来的规则集收紧自己，再叠一小层管连 Unix 套接字的；
//! - 核心按规格缓存规则集（`cache.rs`，指纹 `print.rs`），经 [`crate::command`] 交给助手（`pass.rs`）；
//! - [`available`]、[`confines_sockets`]：探测；[`with_aliases`]：藏起来的另挂在别处的。
//!
//! 只给助手用，不算对外的接口。

mod aliases;
mod cache;
mod pass;
mod print;
mod reads;
mod rules;

pub use aliases::with_aliases;
pub(crate) use cache::{looks, ruleset};
pub use pass::confine_cached;
pub(crate) use pass::inherit;
pub use rules::{available, confine, confines_sockets};
