//! 人生循环叫醒的回合（施工 LF-3，`memory.md` 第一条第 1、2 款）：进回合索引，一条的字只有她这一轮最后一条字不空的回复，
//! 不带叫醒的那段话；她一句没说的不进；别的循环、定时任务叫醒的照旧不进；撤销、恢复、照日志重放、载入时铺回和增量的一样。

use super::*;

const LIFE: &str = r#"{"kind":"schedule","schedule":"t1","loop":"life"}"#;

/// 叫醒的那段话：每次都一样的推演方法。
const WAKE: &str = "推演这一段时间里你过了什么";

#[test]
fn a_turn_the_life_loop_woke_is_put_with_only_her_last_reply() {
    assert_eq!(
        feed(&one_turn(2, LIFE, WAKE, "上午去了趟菜市场，买了条鱼。")),
        [Change::Put(item(3, "上午去了趟菜市场，买了条鱼。"))],
        "只有她最后的回复，时刻是这一轮开始的"
    );
}

#[test]
fn a_life_turn_she_said_nothing_in_is_not_put() {
    let events = [
        said(2, LIFE, WAKE),
        started(3, Some(2)),
        replied(4, 3, ""),
        ended(5, 3, "completed"),
    ];
    assert_eq!(feed(&events), []);
    let events = [
        said(2, LIFE, WAKE),
        started(3, Some(2)),
        ended(4, 3, "interrupted"),
    ];
    assert_eq!(feed(&events), []);
}

#[test]
fn turns_other_loops_and_tasks_woke_are_still_not_put() {
    for by in [
        r#"{"kind":"schedule","schedule":"t1","loop":"dynamic"}"#,
        r#"{"kind":"schedule","schedule":"t1","loop":"fixed"}"#,
        r#"{"kind":"schedule","schedule":"t1"}"#,
    ] {
        assert_eq!(feed(&one_turn(2, by, WAKE, "做完了")), [], "{by}");
    }
}

#[test]
fn a_life_turn_is_removed_on_undo_and_put_back_on_restore() {
    let mut events = one_turn(2, LIFE, WAKE, "去爬了山");
    events.extend(one_turn(7, ALICE, "你今天干嘛了", "爬山去了"));
    events.push(reverted(12, &[3]));
    let changes = feed(&events);
    assert_eq!(changes[2..], [Change::Remove(turn(3))]);
    assert_eq!(replay(&events), [item(8, "你今天干嘛了\n\n爬山去了")]);
    events.push(unreverted(13, &[3]));
    assert_eq!(
        replay(&events),
        [item(3, "去爬了山"), item(8, "你今天干嘛了\n\n爬山去了")],
        "恢复了的照日志放回"
    );
    let (_, changes) = TurnFeed::primed(&events, Some(Seq::new(12).unwrap()));
    assert_eq!(
        changes,
        [
            Change::Restored(vec![turn(3)]),
            Change::Put(item(3, "去爬了山"))
        ],
        "载入时铺回的照同一个判断放回"
    );
}

#[test]
fn replaying_and_priming_give_what_the_feed_gave() {
    let mut events = one_turn(2, LIFE, WAKE, "看了场电影");
    events.extend(one_turn(7, ALICE, "好看吗", "还行"));
    events.extend(one_turn(12, LIFE, WAKE, "睡了个午觉"));
    let put: Vec<TurnItem> = feed(&events)
        .into_iter()
        .filter_map(|change| match change {
            Change::Put(item) => Some(item),
            _ => None,
        })
        .collect();
    assert_eq!(
        put,
        [
            item(3, "看了场电影"),
            item(8, "好看吗\n\n还行"),
            item(13, "睡了个午觉")
        ]
    );
    assert_eq!(replay(&events), put, "照日志重放（补齐、恢复读回）");
    let (_, changes) = TurnFeed::primed(&events, None);
    assert_eq!(
        changes,
        put.into_iter().map(Change::Put).collect::<Vec<_>>(),
        "载入时铺回"
    );
}
