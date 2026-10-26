//! 哪一批要同步到磁盘（施工 V-18，`docs/blueprint/session/actor.md` 第 5 条第 1 款）：会碰外面的要，别的只交给系统。后台同步一串做完、当场交回应（施工 V-19）。
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

/// 数同步了几次的句柄。
struct Counting(std::sync::atomic::AtomicUsize);

impl crate::store::Flush for Counting {
    fn flush(&self) -> std::io::Result<()> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

/// 后台同步一串做完（施工 V-19）：同步到 3 在路上时又要到 5，等第 5 条的回应扣着；派出去以后不经 actor，接着同步到 5、
/// 当场交出回应，都做完了才报回来。
#[tokio::test]
async fn a_sync_chain_answers_on_its_own_and_reports_once_done() {
    use std::sync::{Arc, Mutex};

    use miyu_kernel::id::Seq;
    use miyu_kernel::session::Outcome;

    use super::super::durable::{Syncing, route};
    use super::dispatch;

    let seq = |n| Seq::new(n).expect("从 1 数起");
    let counting = Arc::new(Counting(std::sync::atomic::AtomicUsize::new(0)));
    let shared = Arc::new(Mutex::new(Syncing::new()));
    let (reply, back) = tokio::sync::oneshot::channel();
    {
        let mut syncing = shared.lock().expect("没 panic");
        let flush: Arc<dyn crate::store::Flush> = counting.clone();
        syncing.flush = Some(flush);
        assert_eq!(syncing.durable.written(seq(3), true), Some(seq(3)));
        assert_eq!(syncing.durable.written(seq(5), true), None);
        let accepted = Outcome::Accepted {
            events: vec![seq(5)],
        };
        assert_eq!(route(&mut syncing.durable, reply, accepted), None);
    }
    let (done, mut reported) = tokio::sync::mpsc::unbounded_channel();
    dispatch(Arc::clone(&shared), done, seq(3));
    let (upto, result) = reported.recv().await.expect("报回来了");
    assert!(result.is_ok());
    assert_eq!(upto, seq(5), "接着同步到了 5");
    assert_eq!(counting.0.load(std::sync::atomic::Ordering::Relaxed), 2);
    let back = tokio::time::timeout(std::time::Duration::from_secs(10), back)
        .await
        .expect("十秒内交出回应");
    assert_eq!(
        back.ok(),
        Some(Outcome::Accepted {
            events: vec![seq(5)]
        }),
        "回应交出去了"
    );
    assert!(
        !shared.lock().expect("没 panic").durable.busy(),
        "路上没有了"
    );
}

/// 同步不了的句柄。
struct Broken;

impl crate::store::Flush for Broken {
    fn flush(&self) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Other.into())
    }
}

/// 后台同步出错（施工 V-19）：报回错误，账上记下出过错，扣着的回应放下账时也不交：等的那一头收到会话停了。
#[tokio::test]
async fn a_failed_sync_is_reported_and_answers_nothing() {
    use std::sync::{Arc, Mutex};

    use miyu_kernel::id::Seq;
    use miyu_kernel::session::Outcome;

    use super::super::durable::{Syncing, route};
    use super::dispatch;

    let seq = |n| Seq::new(n).expect("从 1 数起");
    let shared = Arc::new(Mutex::new(Syncing::new()));
    let (reply, back) = tokio::sync::oneshot::channel();
    {
        let mut syncing = shared.lock().expect("没 panic");
        let flush: Arc<dyn crate::store::Flush> = Arc::new(Broken);
        syncing.flush = Some(flush);
        syncing.durable.written(seq(4), true);
        let accepted = Outcome::Accepted {
            events: vec![seq(4)],
        };
        assert_eq!(route(&mut syncing.durable, reply, accepted), None);
    }
    let (done, mut reported) = tokio::sync::mpsc::unbounded_channel();
    dispatch(Arc::clone(&shared), done, seq(4));
    let (_, result) = reported.recv().await.expect("报回来了");
    assert!(result.is_err(), "报了出错");
    assert!(shared.lock().expect("没 panic").failed, "记下出过错");
    drop(shared);
    assert!(back.await.is_err(), "回应没交");
}
