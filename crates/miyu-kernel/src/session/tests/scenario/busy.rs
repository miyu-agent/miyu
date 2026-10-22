//! 场景：她干活的时间（施工 E-M 下，`waits.rs`）。等人批准的那几分钟不算；轮和轮中间闲着的不算；载入以后照日志算出来一样。

use super::*;
use crate::event::Decision;

/// 要人确认的一次写：问一次，允许。
fn asking() -> Verdict {
    Verdict::Ask {
        module: crate::id::ModuleId::parse("permissions").unwrap(),
        access: Access::Write,
        rule: None,
        detail: None,
    }
}

#[test]
fn waiting_for_an_approval_is_not_working_time() {
    let mut stage = stage();
    stage.model([
        Line::calls("我改一下 a。", &[("write", r#"{"path":"a"}"#)]),
        Line::says("改好了。"),
    ]);
    stage.guards([asking()]);
    stage.tools([Play::done("wrote a")]);
    stage.say("改 a");
    stage.advance(10);
    stage.decide(call(6, 1), Decision::Once, None);
    let busy = stage.busy_ms();
    assert!(busy < 60_000, "等人的 10 分钟不算：{busy}");
    // 轮和轮中间闲着的也不算。
    stage.advance(30);
    assert_eq!(stage.busy_ms(), busy);
    // 载入以后照日志算，一样。
    stage.restart();
    assert_eq!(stage.busy_ms(), busy, "{:?}", story(&stage));
}
