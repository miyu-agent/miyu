//! 记忆主动搜只照意思找到的那一路的下限（施工 R-24，`docs/blueprint/recall.md` 第三条第 3 款）：真的 `miyu-embed`、手造的小模型、
//! 真记忆日志。
//!
//! 拿一句关键词一条都撞不上的话去搜，搜到的全是照意思那一路的：表里的模型照表，下限比最像的那条高的一条都不交，比它低的照旧交；
//! 表里没有的模型照 0.40。关键词找到的不看这张表。

use std::sync::Arc;

use miyu_recall::associate::Thresholds;
use miyu_session::{Embedder, Keeper, Listener, Query, RecallTexts, Turn, Using, Vectors};

use crate::support::meaning::*;
use crate::support::package::tiny_package;
use crate::support::*;

const MODEL: &str = "local:tiny";

/// 三条记忆：前半是小词表里的词，后面两个字是这一条独有的。
const MEMORIES: [&str; 3] = ["我的猫 甲乙", "喝茶 丙丁", "hello world 戊己"];

/// 关键词一条都撞不上的问句：小词表里有、三条里都没有的字。
const ASKED: &str = "狗";

/// 照下限表 `table`（没有的不交）接上照意思那一路、记三条、补齐向量。
async fn world(table: Option<&str>) -> (Home, Keeper, Using) {
    let home = Home::new();
    let setup = tiny_package(&home.scratch.0.join("packages").join("embed"));
    let vectors = Vectors::new(Some(Embedder::new(setup)), model_data(&home));
    assert!(home.memory.give_vectors(Arc::new(vectors)));
    if let Some(table) = table {
        let texts = RecallTexts::with(home.resources.path(), Thresholds::default())
            .expect("读得到")
            .searching_near(Thresholds::parse(table).expect("合写法"));
        assert!(home.memory.give_recall(texts));
    }
    for text in MEMORIES {
        save(&home, text);
    }
    let using = Using {
        config: Arc::new(Turn::new(
            Default::default(),
            Arc::clone(&*home.configs.borrow()),
        )),
    };
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    keeper.fill(&using);
    for n in 1..=MEMORIES.len() {
        filled(&home, MODEL, true, &format!("m{n}")).await;
    }
    (home, keeper, using)
}

/// 搜：交回编号。
fn found(keeper: &Keeper, query: &str, near: &Query) -> Vec<String> {
    keeper
        .search(query, false, 5, Some(near))
        .expect("搜得了")
        .iter()
        .map(|entry| entry.id.to_string())
        .collect()
}

/// `query` 的向量，和照意思最像的那一条（编号、多像）。
async fn closest(home: &Home, using: &Using, query: &str) -> (Query, String, f32) {
    let vectors = home.memory.vectors().expect("接上了").clone();
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let near = vectors.query(using, query).await.expect("算得出");
    let hit = log
        .index()
        .nearest(MODEL, &near.vector, 1)
        .expect("找得了")
        .into_iter()
        .next()
        .expect("有一条");
    (near, hit.key, hit.similar)
}

fn table(model: &str, floor: f32) -> String {
    format!("[thresholds]\n\"{model}\" = {floor}\n")
}

#[tokio::test]
async fn only_by_meaning_follows_the_table() {
    let (home, keeper, using) = world(None).await;
    let (near, id, similar) = closest(&home, &using, ASKED).await;
    assert!(
        similar >= miyu_recall::FLOOR,
        "小模型上最像的过 0.40：{similar}"
    );
    assert!(
        keeper
            .search(ASKED, false, 5, None)
            .expect("搜得了")
            .is_empty(),
        "关键词一条都撞不上"
    );
    assert_eq!(
        found(&keeper, ASKED, &near).first(),
        Some(&id),
        "没有下限表：照 0.40，最像的排第一"
    );

    let (_, keeper, _) = world(Some(&table(MODEL, (similar + 0.001).min(1.0)))).await;
    assert_eq!(
        found(&keeper, ASKED, &near),
        Vec::<String>::new(),
        "都不到下限：一条不交"
    );

    let (_, keeper, _) = world(Some(&table(MODEL, similar - 0.001))).await;
    assert_eq!(
        found(&keeper, ASKED, &near),
        vec![id.clone()],
        "过了下限的照交，没过的不交"
    );

    let (_, keeper, _) = world(Some(&table("local:other", 1.0))).await;
    assert_eq!(
        found(&keeper, ASKED, &near).first(),
        Some(&id),
        "表里没有这个模型：照 0.40"
    );
}

#[tokio::test]
async fn a_keyword_hit_ignores_this_table() {
    let (home, _, using) = world(None).await;
    let (near, _, _) = closest(&home, &using, "甲乙").await;
    let (_, keeper, _) = world(Some(&table(MODEL, 1.0))).await;
    assert_eq!(
        found(&keeper, "甲乙", &near),
        vec!["m1".to_string()],
        "关键词找到的照旧"
    );
}

#[test]
fn the_shipped_table_reads() {
    let home = Home::new();
    let text = home.resources.memory_table("near").expect("出厂的读得出来");
    let floors = Thresholds::parse(&text).expect("合写法");
    assert_eq!(
        floors.of("local:granite-embedding-97m-multilingual-r2"),
        Some(GRANITE)
    );
    RecallTexts::load(&home.resources).expect("出厂的几张表读得出来");
}

/// 出厂的 granite 那一行（`core/memory/near.toml`）。
const GRANITE: f32 = 0.80;
