//! 哪一批要同步到磁盘（施工 V-18，`docs/blueprint/session/actor.md` 第 5 条第 1 款）：会碰外面的要，别的只交给系统。
//! 事件照真日志里的样子写（`message.assistant` 的块、`by` 的几种），不照代码拼。

use miyu_kernel::event::Event;

use super::must_sync;

fn event(line: &str) -> Event {
    Event::from_line(line).expect("事件读得出")
}

const MODEL: &str = r#"{"kind":"model","endpoint":"bench","model":"m"}"#;

fn reply(blocks: &str) -> Event {
    event(&format!(
        r#"{{"seq":8,"at":"2026-10-24T14:27:28.028Z","kind":"message.assistant","turn":3,"by":{MODEL},"body":{{"blocks":[{blocks}],"seen":7}}}}"#
    ))
}

fn by_kernel(seq: u64, kind: &str, body: &str) -> Event {
    event(&format!(
        r#"{{"seq":{seq},"at":"2026-10-24T14:27:28.028Z","kind":"{kind}","turn":3,"by":{{"kind":"kernel"}},"body":{body}}}"#
    ))
}

fn tool_result() -> Event {
    event(
        r#"{"seq":10,"at":"2026-10-24T14:27:28.043Z","kind":"tool.result","turn":3,"by":{"kind":"tool","call_id":"call_9_1"},"body":{"call_id":"call_9_1","status":"ok","blocks":[{"type":"text","text":"ok"}]}}"#,
    )
}

fn said() -> Event {
    event(
        r#"{"seq":2,"at":"2026-10-24T14:27:25.766Z","kind":"message.user","by":{"kind":"person","account":"shorin"},"cause":"c","body":{"blocks":[{"type":"text","text":"你好"}]}}"#,
    )
}

#[test]
fn what_the_model_and_tools_write_mid_turn_only_reaches_the_system() {
    // 一轮中间最多的几种：纯文字的回复、工具结果、请求记账、注入的上下文。一批里都是这些的，不同步。
    let batch = [
        reply(r#"{"type":"text","text":"好"}"#),
        by_kernel(
            9,
            "model.called",
            r#"{"seen":7,"endpoint":"bench","model":"m","messages":1,"result":"ok"}"#,
        ),
        tool_result(),
        by_kernel(11, "context.injected", r#"{"kind":"env","text":"x"}"#),
    ];
    assert!(!must_sync(&batch));
    assert!(!must_sync(&[]));
}

#[test]
fn a_reply_that_calls_tools_is_synced_before_they_run() {
    // 落了盘才派工具（`kernel/session.md`「调工具」第 3 条）：断电以后日志里要看得到它派过。
    // 真回复多半先说一句、再调工具：调用不是第一块也算。
    let calls = reply(
        r#"{"type":"text","text":"先跑测试。"},{"type":"tool_call","call_id":"call_8_1","name":"shell","args":"{}"}"#,
    );
    assert!(must_sync(std::slice::from_ref(&calls)));
    // 和不用同步的放在同一批里，整批同步。
    assert!(must_sync(&[tool_result(), calls]));
}

#[test]
fn what_comes_from_outside_is_synced() {
    // 人说的话：回应到了人手里。
    assert!(must_sync(&[said()]));
    // 别的会话交来的（子代理的回报、留言）：那边收到了回应。
    let reported = event(
        r#"{"seq":5,"at":"2026-10-24T14:27:25.766Z","kind":"message.user","by":{"kind":"session","id":"01a0f234-0a1b-7c2d-8e3f-4a5b6c7d8e9f"},"body":{"blocks":[{"type":"text","text":"做完了"}]}}"#,
    );
    assert!(must_sync(&[reported]));
}

#[test]
fn where_a_turn_or_a_session_settles_is_synced() {
    assert!(must_sync(&[by_kernel(
        51,
        "turn.ended",
        r#"{"reason":"completed"}"#
    )]));
    let reverted = by_kernel(52, "turn.reverted", r#"{"turns":[3]}"#);
    assert!(must_sync(&[reverted]));
}
