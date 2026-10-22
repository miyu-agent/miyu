//! 一轮：人说一句、她想了调工具再说话；收尾那一条；翻页和视图流最后一样（施工 9-8 上）。

use miyu_kernel::testkit::{Line, Play};

use crate::support::*;

#[test]
fn a_turn_with_a_thought_a_tool_and_a_reply() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("read", r#"{"file_path":"/home/alice/src/a.rs"}"#)])
            .thinking("look at a"),
        Line::says("Done."),
    ]);
    stage.tools([Play::done("fn main() {}")]);
    stage.say("read a");
    let entries = same(&stage);
    let kinds: Vec<String> = entries
        .iter()
        .map(|e| json(e)["kind"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        kinds,
        ["user", "group", "thought", "tool", "reply", "end"],
        "{entries:#?}"
    );
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(tool["state"], "ok");
    assert_eq!(tool["title"]["name"], "Read");
    assert_eq!(tool["title"]["object"], "~/src/a.rs");
    let group = json(of_kind(&entries, "group")[0]);
    assert_eq!(group["steps"].as_array().map(Vec::len), Some(2));
    assert!(group.get("open").is_none(), "说话了就收起：{group}");
    let first = group["summary_en"][0]["text"].as_str().unwrap_or_default();
    assert!(first.starts_with("Used 1 tool · 1 thought · "), "{group}");
    let end = json(of_kind(&entries, "end")[0]);
    assert_eq!(end["reason"], "completed");
}

#[test]
fn a_reply_before_the_calls_stands_between_two_groups() {
    let mut stage = stage();
    stage.model([
        Line::calls("Let me look.", &[("read", r#"{"file_path":"a"}"#)]),
        Line::calls("", &[("read", r#"{"file_path":"b"}"#)]),
        Line::says("Done."),
    ]);
    stage.tools([Play::done("A"), Play::done("B")]);
    stage.say("look");
    let entries = same(&stage);
    let kinds: Vec<String> = entries
        .iter()
        .map(|e| json(e)["kind"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        kinds,
        ["user", "reply", "group", "tool", "tool", "reply", "end"],
        "说话收起前面那一段，接着的步另起一段；两次请求之间没说话的还在同一段"
    );
}

#[test]
fn a_lone_command_leads_with_its_title_and_a_failure_reds_the_whole_line() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[("shell", r#"{"command":"ls /x","description":"List dirs"}"#)],
        ),
        Line::says("It failed."),
    ]);
    stage.tools([Play::Fails("no such dir".to_string())]);
    stage.say("list");
    let entries = same(&stage);
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(tool["state"], "error");
    assert_eq!(tool["title"]["object"], "List dirs", "命令写短标题");
    let group = json(of_kind(&entries, "group")[0]);
    assert_eq!(group["failed"], true, "只有这一条命令、它出错了：整行红");
    assert_eq!(group["summary_en"][0]["text"], "List dirs · ");
    assert_eq!(
        group["summary_en"][1],
        serde_json::json!({"text": "1 err", "tone": "error"})
    );
}

#[test]
fn an_edit_counts_its_lines() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "edit",
                r#"{"file_path":"a.txt","edits":[{"old_string":"a\n","new_string":"b\nc\n"}]}"#,
            )],
        ),
        Line::says("Edited."),
    ]);
    stage.tools([Play::done("ok")]);
    stage.say("edit");
    let entries = same(&stage);
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(
        tool["diff"],
        serde_json::json!({"added": 2, "removed": 1, "estimated": true})
    );
    let summary = &json(of_kind(&entries, "group")[0])["summary_en"];
    assert_eq!(summary[0]["text"], "Made 1 edit ");
    assert_eq!(
        summary[1],
        serde_json::json!({"text": "+2", "tone": "added"})
    );
    assert_eq!(
        summary[3],
        serde_json::json!({"text": "-1", "tone": "removed"})
    );
}

#[test]
fn only_a_thought_folds_into_thought_for() {
    let mut stage = stage();
    stage.model([Line::says("ok").thinking("hmm")]);
    stage.say("think");
    let entries = same(&stage);
    let summary = &json(of_kind(&entries, "group")[0])["summary_en"];
    let text = summary[0]["text"].as_str().unwrap_or_default();
    assert!(text.starts_with("Thought for "), "{summary}");
    assert_eq!(
        summary.as_array().map(Vec::len),
        Some(1),
        "本来就带时间，不再接用时"
    );
}

#[test]
fn a_broken_reply_drops_the_calls_it_never_finished() {
    use miyu_kernel::event::ErrorClass;
    let mut stage = stage();
    let mut broken = Line::breaks("I'll read", ErrorClass::Auth, "bad key");
    broken.calls = vec![("read".to_string(), r#"{"file_path":"a"}"#.to_string())];
    stage.model([broken]);
    stage.say("read");
    let (_, changes) = live(&stage);
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, miyu_view::Change::Remove { .. })),
        "流式时开了的调用，落了盘没有，拿掉：{changes:#?}"
    );
    let entries = same(&stage);
    assert!(of_kind(&entries, "tool").is_empty());
    assert!(of_kind(&entries, "group").is_empty(), "空了的段也拿掉");
    let end = json(of_kind(&entries, "end")[0]);
    assert_eq!(end["reason"], "error");
    assert_eq!(end["error"]["class"], "auth");
    assert_eq!(end["explain"], "bad key", "供应商的原话照样写");
}

#[test]
fn an_interrupt_cancels_what_has_no_result() {
    use miyu_kernel::session::Queued;
    let mut stage = stage();
    stage.model([Line::calls(
        "",
        &[("shell", r#"{"command":"sleep 9","description":"Wait"}"#)],
    )]);
    stage.tools([Play::done("late").held()]);
    stage.say("wait");
    stage.interrupt(Queued::Send);
    let entries = same(&stage);
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(tool["state"], "cancelled", "{entries:#?}");
    assert_eq!(json(of_kind(&entries, "end")[0])["reason"], "interrupted");
}

#[test]
fn ids_do_not_depend_on_where_feeding_starts() {
    use std::sync::Arc;
    let mut stage = stage();
    stage.model([Line::says("one")]);
    stage.say("first");
    let half = stage.log().len();
    stage.model([
        Line::calls("", &[("read", r#"{"file_path":"a"}"#)]),
        Line::says("two"),
    ]);
    stage.tools([Play::done("A")]);
    stage.say("second");
    let whole: Vec<String> = page(&stage).iter().map(|e| e.id.to_string()).collect();
    let mut projector = miyu_view::Projector::new(Arc::new(texts("en")), None);
    for event in &stage.log()[half..] {
        projector.event(event);
    }
    let tail: Vec<String> = projector
        .entries()
        .iter()
        .map(|e| e.id.to_string())
        .collect();
    assert!(!tail.is_empty());
    assert!(whole.ends_with(&tail), "{whole:?} {tail:?}");
}

/// 回复带这一轮是哪个人格回的（施工 9-8 十三补）：人换了人格先记着，下一回换上快照（带 `policy` 的那一条）起的回复才算新的；
/// 以前的回复、换上之前的那一轮照旧是旧的。无人格的是 `null`。
#[test]
fn a_reply_carries_the_persona_that_spoke_it() {
    let mut stage = stage();
    stage.model([Line::says("one"), Line::says("two"), Line::says("three")]);
    stage.say("first");
    stage.recast(Some("miyu"));
    stage.say("not yet");
    stage.swap_policy(
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        policy,
    );
    stage.say("second");
    let entries = same(&stage);
    let personas: Vec<serde_json::Value> = of_kind(&entries, "reply")
        .iter()
        .map(|reply| json(reply)["persona"].clone())
        .collect();
    assert_eq!(
        personas,
        [
            serde_json::Value::Null,
            serde_json::Value::Null,
            serde_json::json!("miyu")
        ],
        "{entries:#?}"
    );
}

/// 中途换人格、预设（施工 9-8 十四补，终端要的）：记了还没换上的不出旁白，换上的那一刻出一条，换了哪样写哪样。
#[test]
fn a_recast_shows_once_it_takes_effect() {
    let mut stage = stage();
    stage.model([
        Line::says("one"),
        Line::says("two"),
        Line::says("three"),
        Line::says("four"),
        Line::says("five"),
    ]);
    stage.say("first");
    stage.recast(Some("miyu"));
    stage.say("not yet");
    let recasts = |entries: &[miyu_view::Entry]| -> Vec<serde_json::Value> {
        entries
            .iter()
            .map(json)
            .filter(|e| e["what"] == "persona")
            .collect()
    };
    assert!(recasts(&same(&stage)).is_empty(), "记了还没换上");
    stage.swap_policy(
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        policy,
    );
    stage.say("second");
    let shown = recasts(&same(&stage));
    assert_eq!(shown.len(), 1, "{shown:#?}");
    assert_eq!(
        shown[0]["persona"],
        serde_json::json!({"from": null, "to": "miyu"})
    );
    assert!(shown[0].get("preset").is_none(), "只换了人格：{}", shown[0]);
    stage.represet("coding");
    stage.swap_policy(
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        policy,
    );
    stage.say("third");
    let shown = recasts(&same(&stage));
    assert_eq!(shown.len(), 2, "{shown:#?}");
    assert!(
        shown[1].get("persona").is_none(),
        "只换了预设：{}",
        shown[1]
    );
    assert_eq!(shown[1]["preset"]["to"], "coding");
    // 换上之前又换回原来的：等于没换，不出旁白。
    stage.recast(None);
    stage.recast(Some("miyu"));
    stage.represet("writing");
    stage.represet("coding");
    stage.swap_policy(
        "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        policy,
    );
    stage.say("fourth");
    assert_eq!(recasts(&same(&stage)).len(), 2, "换回原样的不算");
}

/// 整理会话的主请求（施工 D-1 下补，网页报的）：`model.called` 带着 `purpose: memory`，照主请求投影，和普通会话的一样：收尾那一条
/// 带模型和用量，思考用时照请求里记的时刻算。
#[test]
fn a_dream_sessions_requests_project_like_main_ones() {
    use miyu_kernel::event::{Body as EventBody, Purpose};
    use miyu_view::Projector;
    let mut stage = stage();
    stage.model([Line::says("ok").thinking("hmm").reports(1200)]);
    stage.say("tidy up");
    let plain = page(&stage);
    let end = json(of_kind(&plain, "end")[0]);
    assert_eq!(
        end["usage"]["uncached"], 1200,
        "普通会话的收尾带用量：{end}"
    );
    let mut projector = Projector::new(std::sync::Arc::new(texts("en")), None);
    let mut marked = 0;
    for event in stage.log() {
        let mut event = event.clone();
        if let EventBody::ModelCalled(called) = &mut event.body {
            called.purpose = Some(Purpose::Memory);
            marked += 1;
        }
        projector.event(&event);
    }
    assert_eq!(marked, 1, "一次主请求");
    assert_eq!(projector.entries(), plain.as_slice());
}
