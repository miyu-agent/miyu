//! 脚本里调的工具（施工 M-5 上，设计 39 第七节第 4 条）：前台那次 `shell` 那一步、后台任务那一行和它了结的旁白带上个数和最近
//! 一个；头重连时照会话交的在跑的补上；点开时的整份照序号。

use std::sync::Arc;

use miyu_kernel::event::JobReason;
use miyu_kernel::id::{ModuleId, NestedId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{NestedRunning, Verdict};
use miyu_kernel::testkit::{Line, Play, Stage, Streamed};
use miyu_kernel::time::Timestamp;
use miyu_view::{Projector, Status};

use crate::support::*;

/// 她跑一个脚本：`shell` 停在跑着。交回那次调用的编号。
fn running_shell(stage: &mut Stage) -> String {
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"python3 x.py","description":"Count"}"#,
            )],
        ),
        Line::says("ok"),
    ]);
    stage.tools([Play::done("done").held()]);
    stage.say("count");
    stage.held_tools()[0].to_string()
}

/// 照视图流的先后喂出来的会话状态：在跑的那一个只有瞬时的开始了。
fn status_of(stage: &Stage) -> Status {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for item in stage.stream() {
        drop(match item {
            Streamed::Event(event) => projector.event(event),
            Streamed::Transient(transient) => projector.transient(transient),
        });
    }
    projector.status()
}

#[test]
fn a_shell_step_counts_its_nested_calls_and_shows_the_latest() {
    let mut stage = stage();
    let shell = running_shell(&mut stage);
    stage.nested_call(
        &format!("{shell}#1"),
        "web_fetch",
        r#"{"url":"https://a.example"}"#,
    );
    // 视图流：开始了就有，在跑的带开始的时刻。
    let (entries, _) = live(&stage);
    let step = json(of_kind(&entries, "tool")[0]);
    assert_eq!(step["nested"]["count"], 1, "{step}");
    assert_eq!(step["nested"]["last"]["state"], "running");
    assert!(step["nested"]["last"]["started"].is_string(), "{step}");
    stage.nested_done(&format!("{shell}#1"), false);
    stage.nested_guards([Verdict::Deny {
        module: ModuleId::parse("permissions").expect("合写法"),
        text: "no".to_string(),
        human: None,
    }]);
    stage.nested_call(
        &format!("{shell}#2"),
        "web_fetch",
        r#"{"url":"https://b.example"}"#,
    );
    stage.nested_call(
        &format!("{shell}#3"),
        "web_fetch",
        r#"{"url":"https://c.example"}"#,
    );
    stage.nested_done(&format!("{shell}#3"), true);
    for call in stage.held_tools() {
        stage.release_tool(call);
    }
    let entries = same(&stage);
    let step = json(of_kind(&entries, "tool")[0]);
    let nested = &step["nested"];
    assert_eq!(nested["count"], 3, "{step}");
    assert_eq!(nested["last"]["n"], 3);
    assert_eq!(nested["last"]["name"], "web_fetch");
    assert_eq!(nested["last"]["state"], "error");
    assert!(
        nested["last"].get("started").is_none(),
        "了结的不带开始的时刻"
    );
    assert_eq!(
        nested["last"]["title"]["object"], "https://c.example",
        "{nested}"
    );
}

#[test]
fn a_shell_step_without_nested_calls_has_no_nested() {
    let mut stage = stage();
    let _ = running_shell(&mut stage);
    for call in stage.held_tools() {
        stage.release_tool(call);
    }
    let entries = same(&stage);
    assert!(json(of_kind(&entries, "tool")[0]).get("nested").is_none());
}

#[test]
fn a_background_command_shows_its_nested_calls_on_its_row_and_its_report() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"python3 x.py","description":"Batch","run_in_background":true}"#,
            )],
        ),
        Line::says("Started."),
        Line::says("Done."),
    ]);
    stage.tools([Play::starts_command(1, "Batch")]);
    stage.say("batch");
    stage.nested_call("j1#1", "web_fetch", r#"{"url":"https://a.example"}"#);
    stage.nested_done("j1#1", false);
    stage.nested_call("j1#2", "web_fetch", r#"{"url":"https://b.example"}"#);
    let row = status_of(&stage).jobs.into_iter().next().expect("有那一行");
    let nested = row.nested.expect("那一行带着");
    assert_eq!((nested.count, nested.last.n), (2, 2));
    // 任务了结时还在跑的记成取消，旁白带最后那一份。
    stage.job_ends(1, JobReason::Exited, By::Kernel, None);
    let entries = same(&stage);
    let notice = json(
        entries
            .iter()
            .find(|e| json(e)["what"] == "job")
            .expect("有任务的旁白"),
    );
    assert_eq!(notice["nested"]["count"], 2, "{notice}");
    assert_eq!(notice["nested"]["last"]["state"], "cancelled");
}

#[test]
fn running_nested_calls_are_filled_in_on_subscribing() {
    let mut stage = stage();
    let shell = running_shell(&mut stage);
    stage.nested_call(
        &format!("{shell}#1"),
        "web_fetch",
        r#"{"url":"https://a.example"}"#,
    );
    // 翻页只有落了盘的：开始了的那一条是瞬时的，没有。
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for event in stage.log() {
        drop(projector.event(event));
    }
    let at = Timestamp::from_unix_millis(1_000).expect("合法的时刻");
    let changes = projector.nested_running(&[NestedRunning {
        id: NestedId::parse(&format!("{shell}#1")).expect("合写法"),
        name: "web_fetch".to_string(),
        args: r#"{"url":"https://a.example"}"#.to_string(),
        at,
    }]);
    assert_eq!(changes.len(), 1, "那一步跟着换：{changes:?}");
    let step = json(
        &projector
            .entries()
            .iter()
            .find(|e| json(e)["kind"] == "tool")
            .expect("有那一步")
            .clone(),
    );
    assert_eq!(step["nested"]["last"]["state"], "running", "{step}");
    assert_eq!(
        step["nested"]["last"]["title"]["object"],
        "https://a.example"
    );
}

#[test]
fn the_full_list_is_by_number_with_the_running_ones() {
    use miyu_kernel::id::{CallId, Parent};
    let mut stage = stage();
    let shell = running_shell(&mut stage);
    stage.nested_guards([Verdict::Deny {
        module: ModuleId::parse("permissions").expect("合写法"),
        text: "outside the workspace".to_string(),
        human: None,
    }]);
    for n in 1..=3 {
        stage.nested_call(
            &format!("{shell}#{n}"),
            "web_fetch",
            &format!(r#"{{"url":"https://{n}.example"}}"#),
        );
    }
    stage.nested_done(&format!("{shell}#2"), false);
    let running = [NestedRunning {
        id: NestedId::parse(&format!("{shell}#3")).expect("合写法"),
        name: "web_fetch".to_string(),
        args: r#"{"url":"https://3.example"}"#.to_string(),
        at: Timestamp::from_unix_millis(1_000).expect("合法的时刻"),
    }];
    let parent = Parent::Call(CallId::parse(&shell).expect("合写法"));
    let texts = texts("en");
    let all = miyu_view::listed(&parent, stage.log(), &running, None, &texts);
    let shown: Vec<(u32, &str)> = all
        .iter()
        .map(|item| (item.n, item.state.as_str()))
        .collect();
    assert_eq!(shown, [(1, "denied"), (2, "ok"), (3, "running")]);
    assert_eq!(all[0].reason.as_deref(), Some("outside the workspace"));
    assert!(all[1].result.is_some(), "成了的带结果的 blob");
    assert_eq!(all[2].title.object.as_deref(), Some("https://3.example"));
    // 只要 2 以后的：3 在跑；在跑的 1 以前的也会带上（这里 1 拒了、不在跑）。
    let after = miyu_view::listed(&parent, stage.log(), &running, Some(2), &texts);
    assert_eq!(after.iter().map(|item| item.n).collect::<Vec<_>>(), [3]);
    // 序号在 `after` 以前、还在跑的照样带上：头手里那一个的状态还会变。
    let later = miyu_view::listed(&parent, stage.log(), &running, Some(5), &texts);
    assert_eq!(later.iter().map(|item| item.n).collect::<Vec<_>>(), [3]);
    let other = Parent::Call(CallId::parse("call_99_1").expect("合写法"));
    assert!(miyu_view::listed(&other, stage.log(), &running, None, &texts).is_empty());
}

/// 先开始的后了结：最近一个照序号，不被早的换回去。
#[test]
fn an_earlier_call_settling_late_does_not_replace_the_latest() {
    let mut stage = stage();
    let shell = running_shell(&mut stage);
    stage.nested_call(
        &format!("{shell}#1"),
        "web_fetch",
        r#"{"url":"https://a.example"}"#,
    );
    stage.nested_call(
        &format!("{shell}#2"),
        "web_fetch",
        r#"{"url":"https://b.example"}"#,
    );
    stage.nested_done(&format!("{shell}#2"), false);
    stage.nested_done(&format!("{shell}#1"), false);
    let entries = page(&stage);
    let step = json(of_kind(&entries, "tool")[0]);
    assert_eq!(step["nested"]["count"], 2, "{step}");
    assert_eq!(step["nested"]["last"]["n"], 2, "{step}");
}

/// 翻页：后台任务下面调过的在切点前，那一行的个数照学过的算。
#[test]
fn a_page_learns_nested_calls_from_before_its_cut() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"python3 x.py","description":"Batch","run_in_background":true}"#,
            )],
        ),
        Line::says("Started."),
    ]);
    stage.tools([Play::starts_command(1, "Batch")]);
    stage.say("batch");
    stage.nested_call("j1#1", "web_fetch", r#"{"url":"https://a.example"}"#);
    stage.nested_done("j1#1", false);
    let log = stage.log();
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    projector.learn(log);
    let row = projector
        .status()
        .jobs
        .into_iter()
        .next()
        .expect("有那一行");
    assert_eq!(row.nested.map(|nested| nested.count), Some(1));
}

/// 脚本里调的要人确认（施工 M-7 上）：卡片记在父 `shell` 那一步上、带序号，`detail` 原样；会话在等它，等的那一件也带序号；
/// 还没开始跑的不进 `nested` 的列表。允许了不再等，照常开始跑。
#[test]
fn a_question_from_a_script_sits_on_the_shell_step() {
    let mut stage = stage();
    let shell = running_shell(&mut stage);
    stage.nested_guards([Verdict::Ask {
        module: ModuleId::parse("permissions").unwrap(),
        access: miyu_kernel::tool::Access::Network,
        rule: None,
        detail: Some(serde_json::from_str(r#"{"tool":"web_fetch"}"#).unwrap()),
    }]);
    stage.nested_call(
        &format!("{shell}#1"),
        "web_fetch",
        r#"{"url":"https://a.example"}"#,
    );
    let (entries, _) = live(&stage);
    let step = json(of_kind(&entries, "tool")[0]);
    assert_eq!(step["approval"]["nested"], 1, "{step}");
    assert_eq!(step["approval"]["detail"]["tool"], "web_fetch");
    assert!(step["approval"].get("decision").is_none(), "还在等");
    assert!(step.get("nested").is_none(), "还没开始跑的不进列表：{step}");
    let waiting = status_of(&stage).waiting;
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].nested, Some(1));
    assert_eq!(waiting[0].call.to_string(), shell);
    stage.decide(waiting[0].call, miyu_kernel::event::Decision::Once, None);
    let (entries, _) = live(&stage);
    let step = json(of_kind(&entries, "tool")[0]);
    assert_eq!(step["approval"]["decision"], "once", "{step}");
    assert_eq!(step["nested"]["last"]["state"], "running");
    assert!(status_of(&stage).waiting.is_empty(), "答了不再等");
}
