//! 有没有在跑的回合（`Handle::busy`，核心照它决定能不能空闲退出）：开回合的那句话的回应到的时候，已经算在跑。以前一批送完才
//! 记，回应在批中间就发出去了，回合开头的挂接点（读配置、人格、技能、召回）慢的那一阵会话算闲着，头一走核心就空闲退出
//! （Windows 的 CI 上 `serve::a_running_turn_keeps_it_running_after_the_head_goes` 三次红）。

use miyu_session::testkit::{Play, Script};

use crate::support::*;

#[tokio::test]
async fn a_turn_counts_as_running_by_the_time_its_message_is_answered() {
    let home = Home::new();
    let script = Script::new([Play::Holds]);
    let handle = home.create(&script).await;
    assert!(!handle.busy(), "刚造出来，没有回合");
    let said = ask(&handle, "m1", say("在吗")).await;
    assert!(said.is_ok(), "{said:?}");
    assert!(handle.busy(), "回应到的时候这一轮已经算在跑");
}
