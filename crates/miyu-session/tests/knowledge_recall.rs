//! 知识库的自动想起（施工 K-6 下，`docs/blueprint/knowledge.md` 第九条）：真会话、真知识库的目录。开了 `auto` 的库，人说的话
//! 提到了某一段标题的名字，这一轮带上那一段；这一段上下文里不再带；平常的话、没开 `auto` 的、预设关了知识库的不带。照意思那一路
//! （远程的假 embedding，门槛表照测试改的那一份）：像过门槛的带、不像的不带；模型不在门槛表里的不算向量；她不搜，段的向量也在
//! 后台补上。原处加了一段的下一轮照文件补。

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, ContextInjected, Event};
use miyu_kernel::session::Command;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::remote_embedding::{MODEL, remote, vector};
use crate::support::resources::copy_resources;
use crate::support::*;

fn catalog(home: &Home) -> Catalog {
    let resources = home.resources.path();
    Catalog::in_packages(vec![(
        miyu_knowledge::PACKAGE,
        miyu_knowledge::tools(resources).expect("读得出"),
    )])
    .expect("合写法")
}

async fn session(home: &Home, turns: usize, lines: Lines) -> Handle {
    let script = Script::new((0..turns).map(|_| Play::Says("好。")));
    home.create_full(&script, &catalog(home), Opening::default(), lines)
        .await
}

/// 说 `words`，等到结束了 `turn` 轮，交回日志。
async fn chat(home: &Home, handle: &Handle, turn: usize, words: &str) -> Vec<Event> {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: None,
    };
    within(
        "回应",
        handle.command(id(&format!("cmd-{turn}")), alice(), said),
    )
    .await
    .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= turn
    })
    .await
}

/// 日志里自动想起交的几块。
fn recalled(log: &[Event]) -> Vec<ContextInjected> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact)
                if fact.kind.as_str() == "recall" && fact.text.starts_with("<knowledge>") =>
            {
                Some(fact.clone())
            }
            _ => None,
        })
        .collect()
}

/// alice 的库 `world`：`kb.toml` 写 `toml`，一篇人物；先照文件补好索引。头一回补在慢的机器上（Windows 的 CI）会把挂接点拖过
/// 300 毫秒，那一轮照规矩不交；这里测的是挑和交，不是头一回补要多久。
fn world(home: &Home, toml: &str, people: &str) {
    let dir = home.root.account_dir(&alice_account()).join("kb/world");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("kb.toml"), toml).expect("写得进");
    std::fs::write(dir.join("people.md"), people).expect("写得进");
    home.knowledge.list(&alice_account()).expect("补得了");
}

const PEOPLE: &str = "# 人物\n\n## 艾琳\n\n守灯人，十九岁。\n\n## 巴德\n\n老船长，缺了一根小指。\n";

#[tokio::test]
async fn a_named_section_comes_once_and_plain_chat_brings_nothing() {
    let home = Home::new();
    world(&home, "auto = true\n", PEOPLE);
    let handle = session(&home, 4, Lines::default()).await;
    let log = chat(&home, &handle, 1, "艾琳今天去哪了").await;
    let blocks = recalled(&log);
    assert_eq!(blocks.len(), 1, "{log:#?}");
    assert_eq!(
        blocks[0].text,
        "<knowledge>\n[world] people.md: 人物 › 艾琳\n守灯人，十九岁。\n</knowledge>\n"
    );
    assert_eq!(blocks[0].refs, ["world/people.md#0"]);
    let log = chat(&home, &handle, 2, "艾琳和巴德呢").await;
    let blocks = recalled(&log);
    assert_eq!(blocks.len(), 2);
    assert_eq!(
        blocks[1].refs,
        ["world/people.md#1"],
        "艾琳这一段上下文里带过了，只带巴德"
    );
    let log = chat(&home, &handle, 3, "今天好累啊").await;
    assert_eq!(recalled(&log).len(), 2, "平常的话不带");
    // 原处加了一段：下一轮照文件补，提到它就带。
    let file = home
        .root
        .account_dir(&alice_account())
        .join("kb/world/people.md");
    std::fs::write(&file, format!("{PEOPLE}\n## 老莫\n\n开杂货铺。\n")).expect("写得进");
    let log = chat(&home, &handle, 4, "去老莫那儿买点东西").await;
    let blocks = recalled(&log);
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[2].refs, ["world/people.md#2"]);
}

#[tokio::test]
async fn a_keyword_under_the_heading_brings_its_section_without_that_line() {
    // 施工 K-9（19 N9，2026-10-14 项目主人定）：标题下一行写关键词，当别名用；带上的字里没有那一行。英文的不分大小写，
    // 标题名也是。
    let home = Home::new();
    world(
        &home,
        "auto = true\n",
        "# 人物\n\n## 艾琳\n关键词：守灯人、灯塔的姑娘、Eileen\n\n守灯人，十九岁。\n\n## Bard\n\n老船长。\n",
    );
    let handle = session(&home, 3, Lines::default()).await;
    let log = chat(&home, &handle, 1, "灯塔的姑娘最近怎么样").await;
    let blocks = recalled(&log);
    assert_eq!(blocks.len(), 1, "{log:#?}");
    assert_eq!(
        blocks[0].text,
        "<knowledge>\n[world] people.md: 人物 › 艾琳\n守灯人，十九岁。\n</knowledge>\n"
    );
    let log = chat(&home, &handle, 2, "bard 呢").await;
    let blocks = recalled(&log);
    assert_eq!(blocks.len(), 2, "标题名不分大小写：{log:#?}");
    assert_eq!(blocks[1].refs, ["world/people.md#1"]);
    let log = chat(&home, &handle, 3, "EILEEN 呢").await;
    assert_eq!(recalled(&log).len(), 2, "带过的不再带");
}

#[tokio::test]
async fn off_bases_and_a_preset_without_knowledge_bring_nothing() {
    let home = Home::new();
    world(&home, "name = \"世界观\"\n", PEOPLE);
    let handle = session(&home, 1, Lines::default()).await;
    let log = chat(&home, &handle, 1, "艾琳今天去哪了").await;
    assert!(recalled(&log).is_empty(), "没开 auto 的不带");

    world(&home, "auto = true\n", PEOPLE);
    let off = miyu_policy::preset::read("[features]\nknowledge = false\n").expect("合写法");
    let lines = Lines {
        preset: Some(miyu_policy::preset::Chosen::new(
            "quiet".to_string(),
            off,
            &miyu_policy::features::Features::default(),
        )),
        ..Lines::default()
    };
    let handle = session(&home, 1, lines).await;
    let log = chat(&home, &handle, 1, "艾琳今天去哪了").await;
    assert!(recalled(&log).is_empty(), "预设关了知识库的不带");
}

/// 库 `world` 每一段都补上远程那个模型的向量：前两段同那一家回的（什么都挺像），后面的跟它垂直（一点都不像）。
fn filled(home: &Home) {
    home.knowledge.list(&alice_account()).expect("补得了");
    let index = home.knowledge.index_of(&alice_account(), "world");
    for (n, key) in index.keys().expect("读得了").iter().enumerate() {
        let vector: [f32; 4] = match n < 2 {
            true => [0.6, 0.8, 0.0, 0.0],
            false => [0.0, 0.0, 1.0, 0.0],
        };
        index.put_vector(key, MODEL, &vector).expect("写得进");
    }
}

#[tokio::test]
async fn by_meaning_over_the_threshold_and_only_for_a_model_in_the_table() {
    let people = "# 人物\n\n## 艾琳\n\n守灯人。\n\n## 巴德\n\n老船长。\n\n## 老莫\n\n开杂货铺。\n\n## 小雀\n\n孤儿。\n";
    // 门槛表里没有这个模型：只走标题名，不算话的向量，也不补段的（远程的不白花钱）。
    let server = Server::start((0..10).map(|_| vector(true)).collect()).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    world(&home, "auto = true\n", people);
    let handle = session(&home, 1, Lines::default()).await;
    let log = chat(&home, &handle, 1, "那个守灯的姑娘怕什么").await;
    assert!(recalled(&log).is_empty());
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(server.received().is_empty(), "不问那一家");

    // 门槛表里有它：像过门槛的带，不像的不带。
    let server = Server::start((0..10).map(|_| vector(true)).collect()).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    let resources = copy_resources(&mut home);
    std::fs::write(
        resources.join("core/knowledge/recall.toml"),
        format!("[thresholds]\n\"{MODEL}\" = 0.5\n"),
    )
    .expect("写得进");
    world(&home, "auto = true\n", people);
    filled(&home);
    let handle = session(&home, 2, Lines::default()).await;
    let mut log = chat(&home, &handle, 1, "那个守灯的姑娘怕什么").await;
    // 头一轮冷：Windows 的 CI 上头一回连那一家、读索引会过 `MEANING`，照规矩这一轮只走标题名（这句话里没有）。同一句再说
    // 一轮，热了以后照意思那一路赶得上；头一轮没交过，第二轮照样交得出。
    if recalled(&log).is_empty() {
        log = chat(&home, &handle, 2, "那个守灯的姑娘怕什么").await;
    }
    let blocks = recalled(&log);
    assert_eq!(blocks.len(), 1, "{log:#?}");
    assert_eq!(
        blocks[0].refs,
        ["world/people.md#0", "world/people.md#1"],
        "像过门槛的带，不像的不带"
    );
    assert!(!server.received().is_empty(), "问句照那一家算");
}

#[tokio::test]
async fn recall_fills_the_vectors_itself() {
    // 她不搜也补得上：开头库里还没有向量，头一轮起了后台补；补上以后下一轮换了说法也带得上。头一轮带不带看谁快（后台补和
    // 照意思那一路并着问同一家），不断言：原来断言头一轮不带，macOS 的 CI 上补得比问得快，偶发红。
    let server = Server::start((0..20).map(|_| vector(true)).collect()).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    let resources = copy_resources(&mut home);
    std::fs::write(
        resources.join("core/knowledge/recall.toml"),
        format!("[thresholds]\n\"{MODEL}\" = 0.5\n"),
    )
    .expect("写得进");
    world(
        &home,
        "auto = true\n",
        "# 人物\n\n## 艾琳\n\n守灯人，最怕打雷。\n",
    );
    let index = home.knowledge.index_of(&alice_account(), "world");
    assert!(
        !index.missing(MODEL, 0, 10).expect("读得了").is_empty(),
        "开头还没有向量"
    );
    let handle = session(&home, 3, Lines::default()).await;
    chat(&home, &handle, 1, "那个守灯的姑娘怕什么").await;
    for _ in 0..600 {
        if index.missing(MODEL, 0, 10).expect("读得了").is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        index.missing(MODEL, 0, 10).expect("读得了").is_empty(),
        "后台补上了"
    );
    let mut log = chat(&home, &handle, 2, "那个守灯的姑娘到底怕什么").await;
    // 这一轮照意思那一路也会冷：Windows 的 CI 上头一回走这条路的那一问会过 `MEANING`（2026-10-22 run 39673453755），照规矩只走
    // 标题名（这句话里没有）。同 `by_meaning_over_the_threshold_and_only_for_a_model_in_the_table`：同一句再说一轮，热了赶得上。
    if recalled(&log).is_empty() {
        log = chat(&home, &handle, 3, "那个守灯的姑娘到底怕什么").await;
    }
    assert_eq!(recalled(&log)[0].refs, ["world/people.md#0"]);
}

#[tokio::test]
async fn names_come_even_when_the_meaning_side_is_slow() {
    // 那一家停住不回（本机模型冷启动的那一轮同理）：照意思那一路等不到，标题名照带，不跟着整块超时。
    let stalled = || Reply {
        status: 200,
        headers: Vec::new(),
        body: vec![Piece::Stall],
    };
    let server = Server::start((0..4).map(|_| stalled()).collect()).await;
    let mut home = Home::new();
    remote(&mut home, &server);
    let resources = copy_resources(&mut home);
    std::fs::write(
        resources.join("core/knowledge/recall.toml"),
        format!("[thresholds]\n\"{MODEL}\" = 0.5\n"),
    )
    .expect("写得进");
    world(&home, "auto = true\n", PEOPLE);
    let handle = session(&home, 1, Lines::default()).await;
    let log = chat(&home, &handle, 1, "艾琳今天去哪了").await;
    let blocks = recalled(&log);
    assert_eq!(blocks.len(), 1, "{log:#?}");
    assert_eq!(blocks[0].refs, ["world/people.md#0"]);
}
