//! 她记下的自己的变化（施工 LF-11，`docs/blueprint/memory.md`「事件」的类、「工具」，设计 `70-推演人生.md` 第六节）：一间真的
//! 记忆日志，直接调 [`Keeper`]。三条路：`note_change` 记、改、去掉（只碰 `change`，现在算数的最多 10 条，改一条不算新记）；她的
//! `remember` 的 `replaces`、`forget`，人经协议改一条，指着 `change` 的拒、什么都不写；人经协议作废、恢复照常，恢复不看上限。
//! 改、作废、恢复一条 `change`，摘要照旧算数。

use miyu_kernel::id::{SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_recall::{CHANGE, CHANGES, Entry, MemoryBook, MemoryEvent, MemoryId, Source, Summary};
use miyu_session::{Keeper, Listener, Stamp};
use miyu_tool::{NoteRefused, Refused, Remember};

use crate::support::meaning::persona;
use crate::support::*;

fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
}

/// 她经工具的那次调用。
fn hers() -> Stamp {
    Stamp {
        at: now(),
        by: serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法"),
        cause: None,
    }
}

/// 人经协议。
fn owners() -> Stamp {
    Stamp {
        at: now(),
        by: alice(),
        cause: None,
    }
}

/// 人生会话的第 3 轮。
fn this_turn() -> Vec<Source> {
    vec![Source {
        session: SessionId::parse("0192f3a0-4444-7abc-8def-001122334455").expect("合写法"),
        turn: TurnId::new(miyu_kernel::id::Seq::new(3).expect("从 1 起")),
    }]
}

/// 她记一条（没有出处：测试场地里没有那个会话的目录，带着出处的算出处死了、不算数，见
/// [`dead_sources_do_not_count`]）。
fn note(home: &Home, text: &str, replaces: Option<MemoryId>) -> Result<MemoryId, NoteRefused> {
    keeper(home).note(hers(), (text.to_string(), None), replaces, Vec::new())
}

fn book<R>(home: &Home, read: impl FnOnce(&MemoryBook) -> R) -> R {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.book(read)
}

fn entry(home: &Home, id: MemoryId) -> Entry {
    book(home, |book| book.get(id).cloned()).expect("有这一条")
}

/// 日志里有几件事：拒了的什么都不写。
fn events(home: &Home) -> usize {
    book(home, |book| book.all().count())
}

/// 人记一条 `user`。
fn user(home: &Home, text: &str) -> MemoryId {
    let remember = Remember {
        class: "user".into(),
        text: text.into(),
        replaces: None,
        keywords: None,
        who: None,
    };
    keeper(home)
        .save(owners(), remember, Vec::new())
        .expect("记得下")
}

/// 验收第 1 条：记一条 `change`，出处是这一轮、`by` 是那次调用；带 `replaces` 的旧的不算了、新的指着它；去掉的作废、为什么
/// 记下。没有是谁、涉及谁、关键词；她写了为什么的记在那一条上。
#[tokio::test]
async fn she_notes_updates_and_drops_her_changes() {
    let home = Home::new();
    let first = keeper(&home)
        .note(
            hers(),
            (
                "以前十一点就睡，这阵子常看书到十二点多".into(),
                Some("连着一周看书看到半夜".into()),
            ),
            None,
            this_turn(),
        )
        .expect("记得下");
    let noted = entry(&home, first);
    assert_eq!(noted.class, CHANGE);
    assert_eq!(noted.sources, this_turn());
    assert!(matches!(noted.by, By::Tool(_)), "{:?}", noted.by);
    assert_eq!((noted.who, noted.keywords), (None, None));
    assert!(noted.people.is_empty());
    assert_eq!(noted.why.as_deref(), Some("连着一周看书看到半夜"));
    let second = note(&home, "这阵子常看书到一点", Some(first)).expect("改得了");
    assert_eq!(entry(&home, first).replaced_by, Some(second));
    assert_eq!(entry(&home, second).replaces, Some(first));
    assert_eq!(entry(&home, second).class, CHANGE);
    assert_eq!(entry(&home, second).why, None, "改一条不带上旧的原因");
    keeper(&home)
        .drop_note(hers(), second, "又早睡了".into())
        .expect("去得掉");
    assert_eq!(entry(&home, second).retired.as_deref(), Some("又早睡了"));
}

/// 验收第 2 条：`replaces`、`drop` 指着没有的、已经不算的、不是 `change` 的，各拒各的，什么都不写。
#[tokio::test]
async fn notes_only_touch_current_changes() {
    let home = Home::new();
    let cat = user(&home, "用户养了一只猫");
    let old = note(&home, "以前爱喝咖啡", None).expect("记得下");
    let new = note(&home, "现在改喝茶", Some(old)).expect("改得了");
    let nothing = MemoryId::parse("m99").expect("合写法");
    let before = events(&home);
    for (id, wanted) in [
        (nothing, NoteRefused::Memory(Refused::NoSuch(nothing))),
        (old, NoteRefused::Memory(Refused::NotCurrent(old))),
        (cat, NoteRefused::NotANote(cat)),
    ] {
        assert_eq!(note(&home, "又变了", Some(id)), Err(wanted.clone()));
        assert_eq!(
            keeper(&home).drop_note(hers(), id, String::new()),
            Err(wanted)
        );
    }
    assert_eq!(events(&home), before, "什么都没写");
    assert!(entry(&home, new).current() && entry(&home, cat).current());
}

/// 验收第 2 条：现在算数的满 10 条，新记的拒；改一条照常成；去掉一条以后又记得下；作废的、改掉的不算。
#[tokio::test]
async fn at_most_ten_are_kept_and_updating_one_is_not_a_new_one() {
    let home = Home::new();
    let first = note(&home, "第 0 处变化", None).expect("记得下");
    let replaced = note(&home, "第 0 处变化，又深了一点", Some(first)).expect("改得了");
    let dropped = note(&home, "淡掉的一处", None).expect("记得下");
    keeper(&home)
        .drop_note(hers(), dropped, "淡了".into())
        .expect("去得掉");
    user(&home, "用户养了一只猫");
    let mut kept = vec![replaced];
    for n in 1..CHANGES {
        kept.push(note(&home, &format!("第 {n} 处变化"), None).expect("还没满"));
    }
    let before = events(&home);
    assert_eq!(
        note(&home, "第 11 处", None),
        Err(NoteRefused::Full(CHANGES))
    );
    assert_eq!(events(&home), before, "什么都没写");
    let updated = note(&home, "第 1 处变化，换了说法", Some(kept[1])).expect("改一条不算新记");
    assert_eq!(entry(&home, kept[1]).replaced_by, Some(updated));
    keeper(&home)
        .drop_note(hers(), updated, "淡了".into())
        .expect("去得掉");
    note(&home, "新的一处", None).expect("去掉一条以后记得下");
}

/// 验收第 4 条：她的 `forget`、`remember` 的 `replaces`，人经协议改一条（`update`，连只换关键词的），指着 `change` 的都拒成同
/// 一种，什么都不写，那一条照旧算数；别的类照旧。
#[tokio::test]
async fn other_ways_in_cannot_touch_a_change() {
    let home = Home::new();
    let change = note(&home, "以前爱喝咖啡，这阵子改喝茶", None).expect("记得下");
    let cat = user(&home, "用户养了一只猫");
    let before = events(&home);
    let keeper = keeper(&home);
    assert_eq!(
        keeper.forget(hers(), change, "不对".into()),
        Err(Refused::Change(change))
    );
    let remember = Remember {
        class: "user".into(),
        text: "改成别的".into(),
        replaces: Some(change),
        keywords: None,
        who: None,
    };
    assert_eq!(
        keeper.save(hers(), remember, this_turn()),
        Err(Refused::Change(change))
    );
    assert_eq!(
        keeper.update(owners(), change, Some("人替她写".into()), None, None),
        Err(Refused::Change(change))
    );
    assert_eq!(
        keeper.update(owners(), change, None, Some("茶".into()), None),
        Err(Refused::Change(change))
    );
    assert_eq!(events(&home), before, "什么都没写");
    assert!(entry(&home, change).current(), "照旧算数");
    keeper
        .forget(hers(), cat, "不养了".into())
        .expect("别的类照旧忘得掉");
}

/// 验收第 4 条：人经协议作废一条 `change`、再恢复，都成；恢复不看上限（恢复以后 11 条照列）。
#[tokio::test]
async fn the_owner_retires_and_restores_a_change_whatever_the_count() {
    let home = Home::new();
    let first = note(&home, "第 0 处变化", None).expect("记得下");
    keeper(&home)
        .retire(owners(), first, "不像她".into())
        .expect("人作废得了");
    assert!(entry(&home, first).retired.is_some());
    for n in 1..=CHANGES {
        note(&home, &format!("第 {n} 处变化"), None).expect("还没满");
    }
    keeper(&home)
        .restore(owners(), first)
        .expect("恢复不看上限");
    assert!(entry(&home, first).current());
    let counted = book(&home, |book| {
        book.all()
            .filter(|entry| entry.class == CHANGE && entry.current())
            .count()
    });
    assert_eq!(counted, CHANGES + 1);
    assert_eq!(
        note(&home, "再来一处", None),
        Err(NoteRefused::Full(CHANGES))
    );
}

/// 验收第 8 条：摘要合到的那一条以前记的 `change`，改、作废、恢复，摘要都照旧算数；摘要里的别的一条作废了照旧作废摘要。
#[tokio::test]
async fn touching_a_change_keeps_the_summary() {
    let home = Home::new();
    let change = note(&home, "以前爱喝咖啡", None).expect("记得下");
    let cat = user(&home, "用户养了一只猫");
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let summary = MemoryEvent::Summary(Summary {
        text: "用户养了一只猫。".into(),
        upto: cat.seq(),
    });
    log.append(now(), alice(), None, &summary).expect("记得下");
    let digest = || {
        book(&home, |book| {
            book.summary().map(|(text, _)| text.to_string())
        })
    };
    let newer = note(&home, "现在改喝茶", Some(change)).expect("改得了");
    assert_eq!(digest().as_deref(), Some("用户养了一只猫。"), "改了");
    keeper(&home)
        .drop_note(hers(), newer, "淡了".into())
        .expect("去得掉");
    keeper(&home).restore(owners(), newer).expect("恢复得了");
    keeper(&home)
        .retire(owners(), newer, "人作废".into())
        .expect("作废得了");
    assert_eq!(
        digest().as_deref(),
        Some("用户养了一只猫。"),
        "作废、恢复了"
    );
    assert!(!book(&home, MemoryBook::summary_stale));
    keeper(&home)
        .retire(owners(), cat, "不养了".into())
        .expect("作废得了");
    assert_eq!(digest(), None, "别的类照旧作废摘要");
}

/// 上限只数出处活着的（同常驻那一块）：出处所在的会话不在了的 `change` 不算，满不了。
#[tokio::test]
async fn dead_sources_do_not_count() {
    let home = Home::new();
    for n in 0..=CHANGES {
        keeper(&home)
            .note(hers(), (format!("第 {n} 处变化"), None), None, this_turn())
            .expect("出处死了的不算数，满不了");
    }
}
