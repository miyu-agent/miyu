//! 联想（施工 R-8，`docs/blueprint/memory.md` 第四条）：真会话、真记忆日志、真的 `miyu-embed` 和手造的小模型（四维，什么都
//! 挺像，门槛由测试自己定）。
//!
//! 人开的一轮：常驻那一块在前，联想那一块在后；带的不在常驻那一块里，最多三条，不超过 500 字节；下一轮不重复带。不过门槛的、
//! 没有照意思那一路的、模型不在门槛表里的、人格记忆没装的，什么都不加。照混合（施工 R-14 补）：字面一个都对不上的不带，门槛再
//! 低也不带。小模型分不出谁更像，断言照性质，不照挑中谁。

use std::collections::BTreeSet;
use std::sync::Arc;

use miyu_kernel::event::{Body, ContextInjected, Event};
use miyu_kernel::id::VenueId;
use miyu_recall::associate::Thresholds;
use miyu_recall::{MemoryEvent, MemoryId, Saved};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Embedder, Keeper, Listener, RecallTexts, Stamp, Turn, Using, Vectors};

use crate::support::meaning::{catalog, chat, filled, model_data, persona, save};
use crate::support::package::tiny_package;
use crate::support::*;

const MODEL: &str = "local:tiny";

/// 接上照意思找的那一路（小模型摆在场地的 `packages/embed/`），联想的门槛表是 `table`。
fn meaning(home: &Home, table: &str) {
    let setup = tiny_package(&home.scratch.0.join("packages").join("embed"));
    let vectors = Vectors::new(Some(Embedder::new(setup)), model_data(home));
    assert!(home.memory.give_vectors(Arc::new(vectors)));
    give(home, table);
}

/// 只交联想的字和门槛表 `table`。
fn give(home: &Home, table: &str) {
    let thresholds = Thresholds::parse(table).expect("合写法");
    let texts = RecallTexts::with(home.resources.path(), thresholds).expect("读得到");
    assert!(home.memory.give_recall(texts));
}

fn table(threshold: &str) -> String {
    format!("[thresholds]\n\"{MODEL}\" = {threshold}\n")
}

/// 人记 `short` 条短的（先记，老的），再记 `long` 条长的（新的）：常驻那一块照新的先列，长的把它占满、短的列不进去。补齐
/// 向量。
async fn remember(home: &Home, short: usize, long: usize) {
    for n in 0..short {
        save(home, &format!("用户喜欢第 {n} 种水果"));
    }
    for n in 0..long {
        save(
            home,
            &format!("第 {n} 条：{}", "用户说过的很长的一件事".repeat(10)),
        );
    }
    if home.memory.vectors().is_none() {
        return;
    }
    let using = Using {
        config: Arc::new(Turn::new(
            Default::default(),
            Arc::clone(&*home.configs.borrow()),
        )),
        owner: alice_account(),
    };
    Keeper::new(&home.memory, persona(), Listener::Owner(alice())).fill(&using);
    for n in 1..=short + long {
        filled(home, MODEL, true, &format!("m{n}")).await;
    }
}

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

/// 第 `turn` 轮（从 1 数）注入的这一类的几块：类别 `kind`（`memory` 是常驻那一块，`recall` 是联想），连同在日志里的序号。
fn blocks(log: &[Event], kind: &str) -> Vec<(u64, ContextInjected)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == kind => {
                Some((event.seq.get(), fact.clone()))
            }
            _ => None,
        })
        .collect()
}

fn refs(fact: &ContextInjected) -> BTreeSet<String> {
    fact.refs.iter().cloned().collect()
}

#[tokio::test]
async fn what_passes_comes_after_the_summary_and_never_twice() {
    let home = Home::new();
    meaning(&home, &table("-1.0"));
    remember(&home, 6, 10).await;
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let log = chat(&home, &handle, 1, ASKED).await;
    let summary = blocks(&log, "memory");
    let recalled = blocks(&log, "recall");
    assert_eq!((summary.len(), recalled.len()), (1, 1), "两块各一");
    let ((at_summary, summary), (at_recall, first)) = (&summary[0], &recalled[0]);
    assert!(at_summary < at_recall, "常驻那一块在前");
    assert!(!first.refs.is_empty() && first.refs.len() <= 3, "{first:?}");
    assert!(first.text.len() <= 500, "{}", first.text.len());
    assert!(first.text.starts_with("<recalled>\n") && first.text.ends_with("\n</recalled>\n"));
    assert!(
        refs(summary).is_disjoint(&refs(first)),
        "常驻那一块里有的不带"
    );
    for id in &first.refs {
        assert!(
            first.text.contains(&format!("{id} user ")),
            "一行照编号、类、日期、正文：{}",
            first.text
        );
    }

    let log = chat(&home, &handle, 2, "还有什么水果").await;
    let recalled = blocks(&log, "recall");
    assert!(blocks(&log, "memory").len() == 1, "常驻那一块不再交");
    assert_eq!(recalled.len(), 2, "第二轮也带：常驻那一块列不进的还有");
    let (_, second) = &recalled[1];
    assert!(refs(second).is_disjoint(&refs(first)), "带过的不再带");
    assert!(refs(second).is_disjoint(&refs(summary)));
}

/// 人说的话：和短的那几条（「用户喜欢第 n 种水果」）字面对得上（「喜欢」「水果」），和长的对不上。
const ASKED: &str = "我喜欢哪种水果";

/// 照 `prepare` 准备好场地、记几条，说一句：交回日志里联想那一块有几块。
async fn recalled_after(prepare: impl FnOnce(&Home), lines: Lines) -> usize {
    recalled_saying(prepare, lines, ASKED).await
}

/// 同 [`recalled_after`]，说的是 `said`。
async fn recalled_saying(prepare: impl FnOnce(&Home), lines: Lines, said: &str) -> usize {
    let home = Home::new();
    prepare(&home);
    remember(&home, 6, 10).await;
    let handle = home
        .create_full(&says(1), &catalog(&home), Opening::default(), lines)
        .await;
    let log = chat(&home, &handle, 1, said).await;
    assert_eq!(blocks(&log, "memory").len(), 1, "常驻那一块照交");
    blocks(&log, "recall").len()
}

#[tokio::test]
async fn nothing_is_brought_unless_it_is_sure() {
    assert_eq!(
        recalled_after(|home| meaning(home, &table("1.0")), Lines::default()).await,
        0,
        "不过门槛的不带"
    );
    assert_eq!(
        recalled_after(
            |home| meaning(home, "[thresholds]\n\"local:other\" = -1.0\n"),
            Lines::default()
        )
        .await,
        0,
        "模型不在门槛表里的不联想"
    );
    assert_eq!(
        recalled_after(|home| give(home, &table("-1.0")), Lines::default()).await,
        0,
        "没有照意思那一路的不联想"
    );
    assert_eq!(
        recalled_saying(
            |home| meaning(home, &table("-1.0")),
            Lines::default(),
            "我养了一只猫"
        )
        .await,
        0,
        "照混合：字面一个都对不上的，门槛再低也不带"
    );
    assert_eq!(
        recalled_unfilled().await,
        0,
        "还没补上向量的不带：说不清多像"
    );
}

/// 接上照意思那一路（门槛 -1），记几条但不补向量，说一句：交回联想那一块有几块。
async fn recalled_unfilled() -> usize {
    let home = Home::new();
    meaning(&home, &table("-1.0"));
    // 同 `remember`：长的把常驻那一块占满，短的列不进去，才轮得到联想；只是一条向量都不补。
    for n in 0..6 {
        save(&home, &format!("用户喜欢第 {n} 种水果"));
    }
    for n in 0..10 {
        save(
            &home,
            &format!("第 {n} 条：{}", "用户说过的很长的一件事".repeat(10)),
        );
    }
    // 先把小程序拉起来：不然这一轮算问句的向量等不到，联想超时，测不出「没向量的不带」。
    let using = Using {
        config: Arc::new(Turn::new(
            Default::default(),
            Arc::clone(&*home.configs.borrow()),
        )),
        owner: alice_account(),
    };
    let vectors = home.memory.vectors().expect("接上了").clone();
    assert!(vectors.query(&using, ASKED).await.is_some(), "算得出");
    let handle = home
        .create_full(
            &says(1),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let log = chat(&home, &handle, 1, ASKED).await;
    blocks(&log, "recall").len()
}

/// 场所会话里也联想（施工 R-13 下）：主人自己的 QQ 私聊（属主就是记忆归的那个账号）是主人那边，照样想起来。场所里只冒出在
/// 那里说过的，和列、搜同一个判法（`Keeper` 的 `shown`，`memory_listener` 测）。
#[tokio::test]
async fn in_the_owners_own_private_chat_too() {
    let lines = Lines {
        venue: VenueId::parse("qq:private:10001").expect("合写法"),
        ..Lines::default()
    };
    assert!(
        recalled_after(|home| meaning(home, &table("-1.0")), lines).await > 0,
        "主人自己的私聊里照样联想"
    );
}

/// 群里也联想主人那边记的（施工 R-29：记忆是一个整体，原来场所里只冒出在那里说过的）：会话的属主不是记忆归的那个账号（系统
/// 账号开的群会话那种），记下时没写场所的照样带上。
#[tokio::test]
async fn in_a_group_what_was_remembered_elsewhere_too() {
    let lines = Lines {
        venue: VenueId::parse("qq:group:5550").expect("合写法"),
        owner_is_admin: true,
        ..Lines::default()
    };
    assert!(
        recalled_after(|home| meaning(home, &table("-1.0")), lines).await > 0,
        "群里也想得起别处记的"
    );
}

/// 只带现在算数的（照 `Keeper::shown`，同搜、同列）：作废了的，字面、意思都对得上也不带。
#[tokio::test]
async fn what_is_forgotten_is_not_brought() {
    let home = Home::new();
    meaning(&home, &table("-1.0"));
    remember(&home, 6, 10).await;
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    for n in 1..=6 {
        let stamp = Stamp {
            at: now(),
            by: alice(),
            cause: None,
        };
        let id = MemoryId::parse(&format!("m{n}")).expect("合写法");
        keeper.retire(stamp, id, "测试".into()).expect("作废得了");
    }
    let handle = home
        .create_full(
            &says(1),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let log = chat(&home, &handle, 1, ASKED).await;
    assert!(blocks(&log, "recall").is_empty(), "作废了的不带");
}

/// 关键词只用来找（施工 R-15 上）：正文里没有、关键词里有的说法也想得起来，带进去的那一块里看不到关键词。
#[tokio::test]
async fn keywords_find_it_but_are_not_shown() {
    let home = Home::new();
    meaning(&home, &table("-1.0"));
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    for n in 0..6 {
        let saved = Saved {
            class: "user".into(),
            text: format!("用户喜欢第 {n} 种水果"),
            sources: Vec::new(),
            audience: vec![alice()],
            replaces: None,
            about: None,
            venue: None,
            keywords: Some("苹果,香蕉".into()),
            who: None,
        };
        log.append(now(), alice(), None, &MemoryEvent::Saved(saved))
            .expect("记得下");
    }
    remember(&home, 0, 10).await;
    for n in 1..=6 {
        filled(&home, MODEL, true, &format!("m{n}")).await;
    }
    let handle = home
        .create_full(
            &says(1),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let log = chat(&home, &handle, 1, "苹果和香蕉").await;
    let recalled = blocks(&log, "recall");
    assert_eq!(recalled.len(), 1, "照关键词对上的想得起来");
    let (_, block) = &recalled[0];
    assert!(
        !block.text.contains("苹果"),
        "关键词不给她看：{}",
        block.text
    );
    assert!(
        block.text.contains("种水果"),
        "给她看的是正文：{}",
        block.text
    );
}

#[tokio::test]
async fn not_without_the_package() {
    let home = Home::new();
    meaning(&home, &table("-1.0"));
    remember(&home, 6, 10).await;
    home.memory.set_installed(false);
    let handle = home
        .create_full(
            &says(1),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let log = chat(&home, &handle, 1, ASKED).await;
    assert!(blocks(&log, "recall").is_empty(), "人格记忆没装的不联想");
}

#[test]
fn the_shipped_texts_and_table_load() {
    let home = Home::new();
    RecallTexts::load(&home.resources).expect("出厂的外壳、门槛表读得出来");
}
