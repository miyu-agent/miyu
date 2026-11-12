//! 确认的截止时刻（施工 CD-1，`docs/blueprint/view.md` 的 `approval`）：要人确认的那一步照请求原样带 `expires_at`，头照它画倒计时；
//! 到时内核替人拒的，`decision` 是 `deny`、`by` 是内核，`expires_at` 照旧留着，翻页和视图流一样。没配等多久的不带。

use miyu_kernel::id::ModuleId;
use miyu_kernel::session::Verdict;
use miyu_kernel::testkit::{Line, Stage};
use miyu_kernel::tool::Access;

use crate::support::*;

/// 她要写一个文件，链问人；这一轮等人确认最多等 `ms` 毫秒。
fn asking(ms: Option<u64>) -> Stage {
    let mut stage = stage();
    stage.confirm_wait(ms);
    stage.model([
        Line::calls("", &[("write", r#"{"file_path":"a","content":"x"}"#)]),
        Line::says("ok"),
    ]);
    stage.guards([Verdict::Ask {
        module: ModuleId::parse("permissions").expect("合写法"),
        access: Access::Write,
        rule: None,
        detail: None,
    }]);
    stage.say("write it");
    stage
}

/// 那一步的 `approval`，翻页的和视图流的要一样。
fn approval(stage: &Stage) -> serde_json::Value {
    let (entries, _) = live(stage);
    let step = json(of_kind(&entries, "tool")[0])["approval"].clone();
    let paged = page(stage);
    assert_eq!(
        json(of_kind(&paged, "tool")[0])["approval"],
        step,
        "翻页和视图流一样"
    );
    step
}

#[test]
fn the_step_carries_the_deadline_and_keeps_it_after_the_kernel_denies() {
    let mut stage = asking(Some(300_000));
    let (deadline, _, _) = stage.expiries()[0];
    let shown = approval(&stage);
    assert_eq!(shown["expires_at"], deadline.to_string(), "{shown}");
    assert!(shown.get("decision").is_none(), "还在等");
    stage.expire(0);
    let shown = approval(&stage);
    assert_eq!(shown["decision"], "deny", "{shown}");
    assert_eq!(shown["by"]["kind"], "kernel");
    assert_eq!(shown["expires_at"], deadline.to_string(), "定了照旧留着");
}

#[test]
fn without_a_wait_there_is_no_deadline() {
    let stage = asking(None);
    let shown = approval(&stage);
    assert!(shown.get("expires_at").is_none(), "{shown}");
}
