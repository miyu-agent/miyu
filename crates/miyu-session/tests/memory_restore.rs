//! 恢复作废的（施工 R-12 下，`docs/blueprint/memory.md` 第二条第 6 款）：听众不合的那一条当没有，恢复不了、什么都不写；听得到
//! 的人恢复得了。别的情形（改掉的、清掉的、本来就算数的、同一个编号）在端点的 `memory_restore` 里测。

use miyu_recall::{MemoryEvent, MemoryId, Retired, Saved};
use miyu_session::{Keeper, Stamp};
use miyu_tool::Refused;

use crate::support::meaning::persona;
use crate::support::*;

#[tokio::test]
async fn only_someone_who_hears_it_can_restore_it() {
    let home = Home::new();
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: "user".into(),
        text: "只有群里那个人听得到的".into(),
        sources: Vec::new(),
        audience: vec![stranger()],
        replaces: None,
        about: None,
    };
    log.append(now(), stranger(), None, &MemoryEvent::Saved(saved))
        .expect("记得下");
    let id = MemoryId::parse("m1").expect("合写法");
    let retired = Retired {
        id,
        why: "错了".into(),
    };
    log.append(now(), stranger(), None, &MemoryEvent::Retired(retired))
        .expect("记得下");
    let stamp = |by| Stamp {
        at: now(),
        by,
        cause: None,
    };
    let retired = || log.book(|book| book.get(id).and_then(|entry| entry.retired.clone()));

    let alices = Keeper::new(&home.memory, persona(), vec![alice()]);
    assert!(
        matches!(alices.restore(stamp(alice()), id), Err(Refused::NoSuch(_))),
        "听不到的当没有"
    );
    assert!(retired().is_some(), "什么都没写");
    let theirs = Keeper::new(&home.memory, persona(), vec![stranger()]);
    theirs.restore(stamp(stranger()), id).expect("恢复得了");
    assert_eq!(retired(), None);
}
