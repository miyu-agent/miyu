//! 后台同步和扣着的回应（施工 V-19）：同步不重叠、几批合成一次、做完接着做；回应等它要的那一条同步了才放，拒绝的、
//! 没有事件的当场交；扣着时后面没排同步的补派一次；停下时一次放完。

use std::sync::Arc;

use tokio::sync::oneshot;

use super::*;
use crate::store::Flush;

fn seq(n: u64) -> Seq {
    Seq::new(n).expect("序号从 1 数起")
}

#[test]
fn syncs_do_not_overlap_and_later_batches_merge_into_the_next_one() {
    let mut durable = Durable::<&str>::default();
    assert!(!durable.busy());
    assert_eq!(
        durable.written(seq(3), true),
        Some(seq(3)),
        "没有在做的，当场派"
    );
    assert!(durable.busy(), "同步在路上");
    assert_eq!(durable.written(seq(5), true), None, "正在做的，只记下");
    assert_eq!(durable.written(seq(8), true), None, "再一批也合进下一次");
    let (released, next) = durable.done(seq(3));
    assert!(released.is_empty());
    assert_eq!(next, Some(seq(8)), "做完一次接着派，到最后写的那一条");
    let (_, next) = durable.done(seq(8));
    assert_eq!(next, None, "没有要再做的");
    assert!(!durable.busy(), "路上没有了");
    assert_eq!(durable.pending(), None, "都同步了");
}

#[test]
fn batches_that_need_no_sync_are_left_to_the_system() {
    let mut durable = Durable::<&str>::default();
    assert_eq!(durable.written(seq(4), false), None);
    assert_eq!(
        durable.pending(),
        Some(seq(4)),
        "写了没同步：停下以前要同步到这里"
    );
    assert_eq!(durable.written(seq(6), true), Some(seq(6)));
    let (_, next) = durable.done(seq(6));
    assert_eq!(next, None);
    assert_eq!(durable.pending(), None);
}

#[test]
fn a_reply_waits_until_its_last_event_is_synced() {
    let mut durable = Durable::default();
    assert_eq!(durable.written(seq(5), true), Some(seq(5)));
    assert_eq!(
        durable.hold(seq(5), "回应"),
        Held::Waiting(None),
        "正在同步到它，不再派"
    );
    let (released, _) = durable.done(seq(4));
    assert!(released.is_empty(), "同步到 4 还不够");
    let (released, _) = durable.done(seq(5));
    assert_eq!(released, ["回应"]);
}

#[test]
fn a_reply_already_synced_goes_out_at_once() {
    let mut durable = Durable::default();
    durable.written(seq(2), true);
    durable.done(seq(2));
    assert_eq!(durable.hold(seq(2), "回应"), Held::Now("回应"));
}

#[test]
fn a_reply_after_a_batch_with_no_sync_asks_for_one() {
    let mut durable = Durable::default();
    assert_eq!(durable.written(seq(7), false), None);
    assert_eq!(
        durable.hold(seq(7), "回应"),
        Held::Waiting(Some(seq(7))),
        "要等的那一条后面没排同步：补派一次"
    );
    // 正在做的不够远：只记下，做完接着做。
    durable.written(seq(9), false);
    assert_eq!(durable.hold(seq(9), "再一个"), Held::Waiting(None));
    let (released, next) = durable.done(seq(7));
    assert_eq!(released, ["回应"]);
    assert_eq!(next, Some(seq(9)));
    let (released, next) = durable.done(seq(9));
    assert_eq!(released, ["再一个"]);
    assert_eq!(next, None);
}

#[test]
fn replies_come_out_in_order_and_only_those_synced() {
    let mut durable = Durable::default();
    durable.written(seq(3), true);
    durable.hold(seq(2), "早的");
    durable.written(seq(6), true);
    durable.hold(seq(6), "晚的");
    let (released, next) = durable.done(seq(3));
    assert_eq!(released, ["早的"], "只放同步到了的");
    assert_eq!(next, Some(seq(6)));
    let (released, _) = durable.done(seq(6));
    assert_eq!(released, ["晚的"]);
}

#[test]
fn stopping_releases_everything_held() {
    let mut durable = Durable::default();
    durable.written(seq(4), true);
    durable.hold(seq(4), "一");
    durable.hold(seq(4), "二");
    assert_eq!(durable.pending(), Some(seq(4)));
    assert_eq!(durable.release_all(), ["一", "二"]);
    assert!(durable.release_all().is_empty());
}

#[test]
fn which_replies_wait_and_for_what() {
    let accepted = Outcome::Accepted {
        events: vec![seq(3), seq(5), seq(4)],
    };
    assert_eq!(waits_for(&accepted), Some(seq(5)), "等最后那一条");
    let empty = Outcome::Accepted { events: Vec::new() };
    assert_eq!(waits_for(&empty), None, "没有事件的当场交");
    let rejected = Outcome::Rejected {
        reason: miyu_kernel::session::Reason::EmptyMessage,
        about: vec![seq(2)],
    };
    assert_eq!(waits_for(&rejected), None, "拒绝的当场交");
    let recapped = Outcome::Recapped {
        text: "回顾".to_string(),
        upto: seq(9),
        cached: false,
    };
    assert_eq!(waits_for(&recapped), None, "回顾那一句不等日志");
}

#[test]
fn a_reply_waiting_for_a_batch_with_no_sync_asks_up_to_the_last_written() {
    let mut durable = Durable::default();
    durable.written(seq(10), false);
    assert_eq!(
        durable.hold(seq(8), "回应"),
        Held::Waiting(Some(seq(10))),
        "补派的同步到写进去的最后一条"
    );
}

/// 一个回应的一头、一尾。
fn channel() -> (oneshot::Sender<Outcome>, oneshot::Receiver<Outcome>) {
    oneshot::channel()
}

#[test]
fn routing_holds_an_accepted_reply_until_synced_and_answers_the_rest_at_once() {
    let mut durable = Durable::default();
    assert_eq!(durable.written(seq(3), true), Some(seq(3)));
    // 接受了、事件还没同步：扣着，不再派（同步在路上）。
    let (reply, mut back) = channel();
    let accepted = Outcome::Accepted {
        events: vec![seq(3)],
    };
    assert_eq!(route(&mut durable, reply, accepted.clone()), None);
    assert!(back.try_recv().is_err(), "还没交");
    // 拒绝的当场交。
    let (reply, mut rejected_back) = channel();
    let rejected = Outcome::Rejected {
        reason: miyu_kernel::session::Reason::EmptyMessage,
        about: Vec::new(),
    };
    assert_eq!(route(&mut durable, reply, rejected.clone()), None);
    assert_eq!(rejected_back.try_recv().ok(), Some(rejected));
    // 同步到了：放出来交给它。
    let (released, _) = durable.done(seq(3));
    for (reply, outcome) in released {
        drop(reply.send(outcome));
    }
    assert_eq!(back.try_recv().ok(), Some(accepted.clone()));
    // 已经同步了的当场交。
    let (reply, mut later) = channel();
    assert_eq!(route(&mut durable, reply, accepted.clone()), None);
    assert_eq!(later.try_recv().ok(), Some(accepted));
}

#[test]
fn routing_a_reply_after_an_unsynced_batch_asks_for_a_sync() {
    let mut durable = Durable::default();
    durable.written(seq(5), false);
    let (reply, mut back) = channel();
    let accepted = Outcome::Accepted {
        events: vec![seq(5)],
    };
    assert_eq!(route(&mut durable, reply, accepted), Some(seq(5)));
    assert!(back.try_recv().is_err());
}

#[test]
fn a_reply_covered_by_the_sync_on_its_way_asks_for_nothing_more() {
    let mut durable = Durable::default();
    assert_eq!(durable.written(seq(5), true), Some(seq(5)));
    // 后面又写了一批不用同步的：回应只要第 5 条，路上那一次就够，做完不再多派一次。
    durable.written(seq(7), false);
    assert_eq!(durable.hold(seq(5), "回应"), Held::Waiting(None));
    let (released, next) = durable.done(seq(5));
    assert_eq!(released, ["回应"]);
    assert_eq!(next, None, "不为它多同步一次");
    assert_eq!(
        durable.pending(),
        Some(seq(7)),
        "第 7 条留给系统，停下以前再同步"
    );
}

/// 假的句柄：数同步了几次，`ok` 是假的同步不了。
struct Counting {
    ok: bool,
    times: std::sync::atomic::AtomicUsize,
}

impl Flush for Counting {
    fn flush(&self) -> std::io::Result<()> {
        self.times
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        match self.ok {
            true => Ok(()),
            false => Err(std::io::ErrorKind::Other.into()),
        }
    }
}

/// 一份扣着一个回应的 `Syncing`：写到第 3 条、同步在路上，句柄照 `ok`。交回它、那个回应的一尾、句柄。
fn holding(ok: bool) -> (Syncing, oneshot::Receiver<Outcome>, Arc<Counting>) {
    let flush = Arc::new(Counting {
        ok,
        times: std::sync::atomic::AtomicUsize::new(0),
    });
    let mut sync = Syncing::new();
    let shared: Arc<dyn Flush> = flush.clone();
    sync.flush = Some(shared);
    sync.durable.written(seq(3), true);
    let (reply, back) = channel();
    let accepted = Outcome::Accepted {
        events: vec![seq(3)],
    };
    assert_eq!(route(&mut sync.durable, reply, accepted), None);
    (sync, back, flush)
}

#[test]
fn leaving_syncs_once_more_and_answers_what_was_held() {
    let (sync, mut back, flush) = holding(true);
    assert!(back.try_recv().is_err(), "扣着");
    drop(sync);
    assert_eq!(
        flush.times.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "退出时再同步一次"
    );
    assert_eq!(
        back.try_recv().ok(),
        Some(Outcome::Accepted {
            events: vec![seq(3)]
        }),
        "同步成了照接受交出去"
    );
}

#[test]
fn leaving_after_a_failed_sync_answers_nothing() {
    let (sync, mut back, _) = holding(false);
    drop(sync);
    assert!(
        back.try_recv().is_err(),
        "同步不了的不交：等的那一头收到会话停了"
    );
    let (mut sync, mut back, flush) = holding(true);
    sync.failed = true;
    drop(sync);
    assert!(back.try_recv().is_err(), "同步出过错的不交");
    assert_eq!(
        flush.times.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "也不再同步"
    );
}

#[test]
fn only_replies_to_the_outside_are_held() {
    use miyu_kernel::id::{AccountId, CallId, SessionId};
    use miyu_kernel::origin::{Person, Session, Tool};
    let person = By::Person(Person::new(AccountId::parse("alice").expect("合写法")));
    assert!(outside(&person), "人发的扣着");
    let session = By::Session(Session {
        id: SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91").expect("合写法"),
    });
    assert!(!outside(&session), "别的会话发的当场交");
    assert!(!outside(&By::Kernel));
    let tool = By::Tool(Tool {
        call_id: CallId::parse("call_1_1").expect("合写法"),
    });
    assert!(!outside(&tool));
}
