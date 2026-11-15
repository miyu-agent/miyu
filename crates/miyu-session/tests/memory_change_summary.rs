//! 常驻那一块里她的变化（施工 LF-11，`docs/blueprint/memory.md` 第三条第 10 款）：真会话、真记忆日志。现在算数的 `change` 不管
//! 摘要合到哪都列，在开头那一行后面、摘要那一行前面，标题一行、旧的在前；不占 3000 字节、不算进截了的条数、不在摘要以后新记的
//! 那一段里再列；`refs` 里有它们。一条都没有的（作废了的也算没有）和没有这一类时逐字节一样；只有它们的照样交。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::time::Timestamp;
use miyu_recall::{MemoryEvent, MemoryId, Merged, Retired, Saved, Summary};
use miyu_session::testkit::{Play, Script};
use miyu_store::recall::Room;

use crate::support::*;

/// `software/memory/summary/changes.txt`。
const TITLE: &str = "How I have changed over this life, noted by me:";

fn persona() -> Room {
    Room::persona("engineer")
}

/// 记下的时刻：会话的时区（东九区）里是 10 月 8 日。
fn saved_at() -> Timestamp {
    Timestamp::parse("2026-10-07T20:00:00.000Z").expect("合写法")
}

fn append(home: &Home, by: By, event: MemoryEvent) -> MemoryId {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.append(saved_at(), by, None, &event).expect("记得下").id
}

fn saved(class: &str, text: &str, replaces: Option<MemoryId>) -> MemoryEvent {
    MemoryEvent::Saved(Saved {
        class: class.into(),
        text: text.into(),
        sources: Vec::new(),
        audience: vec![alice()],
        replaces,
        about: None,
        venue: None,
        keywords: None,
        who: None,
        people: Vec::new(),
        place: None,
        why: None,
        imported: None,
    })
}

/// 人记一条 `user`。
fn user(home: &Home, text: &str) -> MemoryId {
    append(home, alice(), saved("user", text, None))
}

/// 她记一条变化（经工具的那次调用）。
fn change(home: &Home, text: &str) -> MemoryId {
    let by = serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法");
    append(home, by, saved("change", text, None))
}

fn retire(home: &Home, id: MemoryId) {
    let retired = Retired {
        id,
        why: "淡了".into(),
        kind: None,
        kept: None,
    };
    append(home, alice(), MemoryEvent::Retired(retired));
}

/// 合并写了摘要 `text`、合到 `upto`。
fn summarized(home: &Home, upto: MemoryId, text: &str) {
    let module: By = serde_json::from_str(r#"{"kind":"module","id":"memory"}"#).expect("合写法");
    let summary = Summary {
        text: text.into(),
        upto: upto.seq(),
    };
    append(home, module.clone(), MemoryEvent::Summary(summary));
    let merged = Merged {
        upto: upto.seq(),
        given: 1,
        revised: 0,
        retired: 0,
        failed: false,
    };
    append(home, module, MemoryEvent::Merged(merged));
}

fn line(id: MemoryId, class: &str, text: &str) -> String {
    format!("{id} {class} 2026-10-08: {text}")
}

fn summaries(request: &Request) -> Vec<String> {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } => blocks.clone(),
            _ => Vec::new(),
        })
        .filter_map(|block| match block {
            Block::Text(Text { text }) if text.starts_with("<memories>") => Some(text),
            _ => None,
        })
        .collect()
}

/// 开一个会话说一句：那一次请求里常驻那一块和日志里记下的 `refs`；没交的没有。
async fn first_block(home: &Home) -> Option<(String, Vec<String>)> {
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("在吗")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let block = summaries(&script.requests()[0].1).into_iter().next();
    let refs = home
        .log(handle.id())
        .iter()
        .find_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "memory" => {
                Some(fact.refs.clone())
            }
            _ => None,
        });
    stop(&handle).await;
    block.zip(refs)
}

/// 验收第 5 条：两条 `change`（一条在摘要合到的以前，一条在以后）、一份摘要、摘要以后新记的三条：标题、两条旧的在前、摘要、
/// 三条新的在前；`change` 不在新记的那一段里再列；`refs` 里两条 `change` 在前。
#[tokio::test]
async fn changes_come_first_oldest_first_then_the_summary_and_what_came_after() {
    let home = Home::new();
    let early = change(&home, "以前十一点就睡，这阵子常看书到十二点多");
    let cat = user(&home, "用户养了一只猫");
    summarized(&home, cat, "用户养了一只猫。");
    let tokyo = user(&home, "用户搬到了东京");
    let late = change(&home, "以前爱喝咖啡，这个月改喝茶");
    let rust = user(&home, "用户周末写 Rust");
    let go = user(&home, "用户在学 Go");
    let (block, refs) = first_block(&home).await.expect("交了");
    assert_eq!(
        block,
        [
            "<memories>".to_string(),
            TITLE.to_string(),
            line(early, "change", "以前十一点就睡，这阵子常看书到十二点多"),
            line(late, "change", "以前爱喝咖啡，这个月改喝茶"),
            "What I know: 用户养了一只猫。".to_string(),
            line(go, "user", "用户在学 Go"),
            line(rust, "user", "用户周末写 Rust"),
            line(tokyo, "user", "用户搬到了东京"),
            "</memories>\n".to_string(),
        ]
        .join("\n")
    );
    assert_eq!(
        refs,
        [early, late, go, rust, tokyo].map(|id| id.to_string()),
        "refs 里有它们，摘要那一行不算"
    );
}

/// 验收第 5 条：别的条目多到截断时，`change` 一条不少，截的句数只算别的：整块就是没有 `change` 的那一块在开头那一行后面插进
/// 标题和那几条。
#[tokio::test]
async fn changes_never_take_room_from_the_rest() {
    let fill = |home: &Home| {
        for n in 0..40 {
            user(home, &format!("第 {n} 条：{}", "很长的一句话".repeat(5)));
        }
    };
    let plain = Home::new();
    fill(&plain);
    let (without, plain_refs) = first_block(&plain).await.expect("交了");
    assert!(
        without.contains("more, found by memory_search."),
        "截了：{without}"
    );

    let home = Home::new();
    fill(&home);
    let texts: Vec<String> = (0..10)
        .map(|n| format!("第 {n} 处变化：{}", "以前怎样，现在怎样".repeat(4)))
        .collect();
    let ids: Vec<MemoryId> = texts.iter().map(|text| change(&home, text)).collect();
    let (block, refs) = first_block(&home).await.expect("交了");
    let rows: Vec<String> = ids
        .iter()
        .zip(&texts)
        .map(|(id, text)| line(*id, "change", text))
        .collect();
    let wanted = without.replacen(
        "<memories>\n",
        &format!("<memories>\n{TITLE}\n{}\n", rows.join("\n")),
        1,
    );
    assert_eq!(block, wanted);
    assert!(block.len() > 3000, "她的变化不占上限：{}", block.len());
    let mut wanted_refs: Vec<String> = ids.iter().map(ToString::to_string).collect();
    wanted_refs.extend(plain_refs);
    assert_eq!(refs, wanted_refs);
}

/// 验收第 5 条：没有算数的 `change`（只有作废了的）的，和没有这一类时逐字节一样。
#[tokio::test]
async fn without_a_current_change_the_block_is_as_before() {
    let plain = Home::new();
    let cat = user(&plain, "用户养了一只猫");
    let (without, _) = first_block(&plain).await.expect("交了");
    assert_eq!(
        without,
        format!(
            "<memories>\n{}\n</memories>\n",
            line(cat, "user", "用户养了一只猫")
        )
    );
    let home = Home::new();
    let gone = change(&home, "以前爱喝咖啡");
    retire(&home, gone);
    let cat = user(&home, "用户养了一只猫");
    let (block, refs) = first_block(&home).await.expect("交了");
    assert_eq!(
        block,
        format!(
            "<memories>\n{}\n</memories>\n",
            line(cat, "user", "用户养了一只猫")
        )
    );
    assert_eq!(refs, [cat.to_string()]);
    let empty = Home::new();
    let gone = change(&empty, "以前爱喝咖啡");
    retire(&empty, gone);
    assert!(first_block(&empty).await.is_none(), "一条都没有的不交");
}

/// 验收第 5 条：只有 `change`、没有别的条目也没有摘要的照样交，只有标题和那几条；改掉的只列新的那一版。
#[tokio::test]
async fn only_changes_still_make_a_block() {
    let home = Home::new();
    let first = change(&home, "以前爱喝咖啡");
    let by = serde_json::from_str(r#"{"kind":"tool","call_id":"call_4_1"}"#).expect("合写法");
    let newer = append(
        &home,
        by,
        saved("change", "以前爱喝咖啡，这个月改喝茶", Some(first)),
    );
    let sleep = change(&home, "以前十一点就睡");
    let (block, refs) = first_block(&home).await.expect("交了");
    assert_eq!(
        block,
        format!(
            "<memories>\n{TITLE}\n{}\n{}\n</memories>\n",
            line(newer, "change", "以前爱喝咖啡，这个月改喝茶"),
            line(sleep, "change", "以前十一点就睡")
        )
    );
    assert_eq!(refs, [newer.to_string(), sleep.to_string()]);
}
