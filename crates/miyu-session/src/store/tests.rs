use miyu_kernel::event::Event;
use miyu_kernel::id::{Seq, SessionId};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};

use super::{Base, Indexed, Store};

/// 第 `n` 号事件：一句话。
fn said(n: u64) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{n},"at":"2026-10-11T07:00:00.000Z","kind":"message.user","by":{{"kind":"person"}},"body":{{"blocks":[{{"type":"text","text":"第 {n} 句"}}]}}}}"#
    ))
    .unwrap()
}

fn seqs(events: &[Event]) -> Vec<u64> {
    events.iter().map(|event| event.seq.get()).collect()
}

/// 分叉出来的会话读回日志（施工 C-8 上）：先接底子、再接自己的；从分叉点以前读起的（撤销读回到压缩以前）照样接得上。底子
/// 照父会话的日志现读（施工 V-20）：父会话过了分叉点又说的不算进来。
#[test]
fn a_forked_log_reads_back_with_its_base_first() {
    let root = std::env::temp_dir().join(format!("miyu-session-store-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&root));
    // 会话日志只在数据根里建目录（施工 TMP-1）：写上标记，日志放在下面一层。
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(miyu_store::root::MARKER), "").unwrap();
    let parent = root.join("parent");
    let mut parent_log = SessionLog::create_at(&parent, SEGMENT_LIMIT, Seq::FIRST).unwrap();
    parent_log
        .append(&[said(1), said(2), said(3), said(4), said(5)])
        .unwrap();
    let dir = root.join("session");
    let mut log = SessionLog::create_at(&dir, SEGMENT_LIMIT, Seq::new(4).unwrap()).unwrap();
    log.append(&[said(4), said(5)]).unwrap();
    let id = SessionId::parse("01a0f234-0a1b-7c2d-8e3f-4a5b6c7d8e9f").unwrap();
    let base = Base::new(parent.clone(), Seq::new(3).unwrap());
    let store = Indexed::new(log, &id, None, None, None).on_base(base);
    assert_eq!(seqs(&store.events().unwrap()), [1, 2, 3, 4, 5]);
    assert_eq!(
        seqs(&store.events_from(Seq::new(2).unwrap()).unwrap()),
        [2, 3, 4, 5]
    );
    assert_eq!(seqs(&store.events_from(Seq::new(5).unwrap()).unwrap()), [5]);
    // 父会话接着说，底子不变。
    parent_log.append(&[said(6)]).unwrap();
    assert_eq!(seqs(&store.events().unwrap()), [1, 2, 3, 4, 5]);
    drop(std::fs::remove_dir_all(&root));
}

/// 底子读到分叉点就停，从哪条起照 `from`（施工 V-20）；父会话的日志没了的报错，不当成没有底子；从分叉点以后读起的不碰
/// 父会话的日志。
#[test]
fn a_base_reads_its_parent_up_to_the_fork_and_fails_without_it() {
    let root = std::env::temp_dir().join(format!("miyu-session-base-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&root));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(miyu_store::root::MARKER), "").unwrap();
    let parent = root.join("parent");
    let mut parent_log = SessionLog::create_at(&parent, SEGMENT_LIMIT, Seq::FIRST).unwrap();
    parent_log.append(&[said(1), said(2), said(3)]).unwrap();
    let base = Base::new(parent.clone(), Seq::new(2).unwrap());
    assert_eq!(seqs(&base.events_from(Seq::FIRST).unwrap()), [1, 2]);
    assert_eq!(seqs(&base.events_from(Seq::new(2).unwrap()).unwrap()), [2]);
    assert!(base.events_from(Seq::new(3).unwrap()).unwrap().is_empty());
    drop(parent_log);
    drop(std::fs::remove_dir_all(&parent));
    assert!(base.events_from(Seq::FIRST).is_err());
    assert!(base.events_from(Seq::new(3).unwrap()).unwrap().is_empty());
    drop(std::fs::remove_dir_all(&root));
}
