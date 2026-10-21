//! 抽取不把忘掉的抽回来（施工 R-6 修，`docs/blueprint/memory.md` 第六条第 4 款）：这一段开始以后作废的、改掉的，接在这一段
//! 后面列出来，新的在前、最多 20 条；这一段以前就作废的、清掉的不列；一条都没有的请求一字不变。整理记忆的模型是假服务器。

use miyu_http::testkit::Server;
use miyu_recall::{Cleared, MemoryEvent, MemoryId, Retired, Saved};
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, persona};
use crate::support::*;

/// `software/memory/extract/dropped.txt`。
const DROPPED: &str = "These memories were forgotten or changed after this conversation, so what the conversation says about them is out of date. Do not record them again:";

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

/// 人记一条（改的写 `replaces`），交回编号。
fn save(home: &Home, text: &str, replaces: Option<MemoryId>) -> MemoryId {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: vec![alice()],
        replaces,
        about: None,
        venue: None,
        keywords: None,
        who: None,
    };
    log.append(now(), alice(), None, &MemoryEvent::Saved(saved))
        .expect("记得下")
        .id
}

/// 人作废一条。
fn retire(home: &Home, id: MemoryId) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let retired = Retired {
        id,
        why: "不算了".into(),
        kind: None,
        kept: None,
    };
    log.append(now(), alice(), None, &MemoryEvent::Retired(retired))
        .expect("记得下");
}

/// 请求里 `DROPPED` 那一句后面的几行；没有那一句的没有。
fn dropped(text: &str) -> Option<Vec<String>> {
    let (_, after) = text.split_once(&format!("\n\n{DROPPED}\n"))?;
    Some(after.lines().map(str::to_string).collect())
}

/// 一行照 `memory_search/memory.txt`：`m12 user <日期>: 正文`。
fn names(line: &str, id: MemoryId, text: &str) -> bool {
    line.starts_with(&format!("{id} user ")) && line.ends_with(&format!(": {text}"))
}

#[tokio::test]
async fn what_was_dropped_after_the_slice_began_follows_it() {
    let server = Server::start(vec![every_turn("不要紧")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let moved = save(&home, "用户以前住在大阪", None);
    retire(&home, moved);
    let checkup = save(&home, "用户下周三上午去体检", None);
    let cat = save(&home, "用户养了一只猫", None);
    save(&home, "用户喜欢咖啡", None);
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "下周三我要干嘛").await;
    // 这一段开始以后：作废一条、改一条。
    retire(&home, checkup);
    save(&home, "用户养了两只猫", Some(cat));
    chat(&home, &handle, 2, "好的").await;
    requested(&server, 1).await;
    let text = asked(&server, 0);
    let lines = dropped(&text).unwrap_or_else(|| panic!("接在这一段后面：{text}"));
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        names(&lines[0], cat, "用户养了一只猫"),
        "新的在前：{lines:?}"
    );
    assert!(
        names(&lines[1], checkup, "用户下周三上午去体检"),
        "{lines:?}"
    );
    assert!(!text.contains("大阪"), "这一段以前就作废的不列");
    assert!(
        !text.contains("咖啡") && !text.contains("两只猫"),
        "还算数的不列"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn nothing_dropped_leaves_the_request_as_it_was() {
    let server = Server::start(vec![every_turn("不要紧"), every_turn("不要紧")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let moved = save(&home, "用户以前住在大阪", None);
    retire(&home, moved);
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "你好").await;
    chat(&home, &handle, 2, "好的").await;
    requested(&server, 1).await;
    let text = asked(&server, 0);
    assert!(!text.contains(DROPPED), "{text}");
    assert!(text.ends_with("</turn>"), "{text}");
    stop(&handle).await;

    // 清掉的不列：清空是另一回事。
    let server = Server::start(vec![every_turn("不要紧")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let checkup = save(&home, "用户下周三上午去体检", None);
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "你好").await;
    retire(&home, checkup);
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.append(
        now(),
        alice(),
        None,
        &MemoryEvent::Cleared(Cleared { session: None }),
    )
    .expect("记得下");
    chat(&home, &handle, 2, "好的").await;
    requested(&server, 1).await;
    assert!(!asked(&server, 0).contains(DROPPED), "清掉的不列");
    stop(&handle).await;
}

#[tokio::test]
async fn at_most_twenty_are_listed() {
    let server = Server::start(vec![every_turn("不要紧")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let ids: Vec<MemoryId> = (0..21)
        .map(|n| save(&home, &format!("用户的第 {n} 件事"), None))
        .collect();
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "你好").await;
    for id in &ids {
        retire(&home, *id);
    }
    chat(&home, &handle, 2, "好的").await;
    requested(&server, 1).await;
    let text = asked(&server, 0);
    let lines = dropped(&text).expect("有");
    assert_eq!(lines.len(), 20, "{lines:?}");
    assert!(names(&lines[0], ids[20], "用户的第 20 件事"), "新的在前");
    assert!(!text.contains("用户的第 0 件事"), "最早忘掉的那一条放不下");
    stop(&handle).await;
}
