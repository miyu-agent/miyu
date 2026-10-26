//! 恢复作废的（施工 R-12 下，`docs/blueprint/memory.md` 第二条第 6 款）：这里冒不出来的那一条当没有，恢复不了、什么都不写；冒得
//! 出来的地方恢复得了（施工 R-13 上：在那个场所，或者主人那边）。别的情形（改掉的、清掉的、本来就算数的、同一个编号）在端点的
//! `memory_restore` 里测。

use miyu_kernel::id::VenueId;
use miyu_recall::{MemoryEvent, MemoryId, Retired, Saved};
use miyu_session::{Keeper, Listener, Stamp};

use crate::support::meaning::persona;
use crate::support::*;

fn place(n: u32) -> VenueId {
    VenueId::parse(&format!("qq:group:{n}")).expect("合写法")
}

#[tokio::test]
async fn it_can_be_restored_anywhere() {
    let home = Home::new();
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: "user".into(),
        text: "一群里说的".into(),
        sources: Vec::new(),
        audience: Vec::new(),
        replaces: None,
        about: None,
        venue: Some(place(1)),
        keywords: None,
        who: None,
        people: Vec::new(),
    };
    log.append(now(), stranger(), None, &MemoryEvent::Saved(saved))
        .expect("记得下");
    let id = MemoryId::parse("m1").expect("合写法");
    let retire = || {
        let retired = Retired {
            id,
            why: "错了".into(),
            kind: None,
            kept: None,
        };
        log.append(now(), stranger(), None, &MemoryEvent::Retired(retired))
            .expect("记得下");
    };
    retire();
    let stamp = || Stamp {
        at: now(),
        by: alice(),
        cause: None,
    };
    let retired = || log.book(|book| book.get(id).and_then(|entry| entry.retired.clone()));

    let elsewhere = Keeper::new(&home.memory, persona(), Listener::Place(place(2)));
    elsewhere
        .restore(stamp(), id)
        .expect("别的群里也恢复得了（施工 R-29：记忆是一个整体）");
    assert_eq!(retired(), None);
    retire();
    let there = Keeper::new(&home.memory, persona(), Listener::Place(place(1)));
    there.restore(stamp(), id).expect("在那个群里恢复得了");
    assert_eq!(retired(), None);
    retire();
    let owner = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    owner.restore(stamp(), id).expect("主人那边恢复得了");
    assert_eq!(retired(), None);
}
