//! 长期目标这个软件包（施工 X-8 上，`docs/blueprint/goal.md`，工具一页 `tools/goal.md`）：她用来看、设、结束这个会话的长期目标
//! 的工具 [`goal`]。不放进基础系统：长期目标是能关的软件包（设计 `30-插件框架.md` 第二节的表）。续轮由会话层开
//! （`miyu-session` 的 `actor/goal.rs`），这里只有工具。
//!
//! 工具只拿 [`miyu_tool::GoalPort`]：目标的状态、内核都在端口后面（实现在 `miyu-session`）。给模型看的字在资源目录的
//! `software/goal/` 下（进登记簿），给人看的在 `software/goal/human/`。
//!
//! [`goal`]: miyu_tool::GOAL

mod goal;
mod texts;

use std::path::Path;
use std::sync::Arc;

use miyu_tool::Tool;
use miyu_tool::load::LoadError;

/// 这个软件包的编号：资源目录里的名字，也是预设里开关它的那个键。
pub const PACKAGE: &str = "goal";

/// 这个包的工具：只有 `goal`。字从资源目录 `resources` 读。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对。
pub fn tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    Ok(vec![Arc::new(goal::Goal::load(resources)?)])
}
