//! 测试用的执行器替身（`docs/designs/02-内核.md` 第四节「执行器怎么回动作」）：照剧本回模型、
//! 回工具、回执行前的链、回挂接点，替会话落盘，把真会话一整轮一整轮地跑完。
//!
//! 只在 `testkit` 开关打开时编进去，内核自己的测试里总是有（`01-架构.md` 第九节）。它不做 I/O：
//! 「磁盘」是一串内存里的事件，「模型」「工具」照剧本回。
//!
//! - [`Line`]：模型的一次回复；
//! - [`Play`]：一次工具调用怎么回；
//! - [`Stage`]：替身本身，人的每个动作以后一直跑到没事可做；子会话、执行器交来的回报也照样跑（施工 7-2，`jobs.rs`）；别的
//!   会话发来的话也是（施工 C-2，`peers.rs`）；替它看图的转述照剧本回（施工 8-17，`sight.rs`）；

mod aside;
mod compact;
mod disk;
mod jobs;
mod limits;
mod opening;
mod peers;
mod prepare;
mod respond;
mod routing;
mod script;
mod sight;
mod stage;
mod stream;
mod swap;

pub use opening::{CHILD_SESSION, SESSION};
pub use respond::model;
pub use script::{Line, Play};
pub use sight::vision_model;
pub use stage::Stage;
pub use stream::Streamed;
