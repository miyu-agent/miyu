//! 记忆主动搜的关键词下限（施工 R-14，`docs/blueprint/recall.md` 第三条第 3 款）：真的 `miyu-embed`、手造的小模型、真记忆日志。
//!
//! 小模型四维、什么都挺像，下限照它量着定：挑一条照意思排在后面的（她搜一条要 `limit` 的四倍，排不进的只有关键词那一路找得到），
//! 拿它独有的两个字去搜。表里的模型、下限比它高的不要它；下限比它低的、表里没有这个模型的、这个模型还没有它的向量的，照旧找得到。

use std::sync::Arc;

use miyu_recall::associate::Thresholds;
use miyu_session::{Embedder, Keeper, Listener, Query, RecallTexts, Turn, Using, Vectors};

use crate::support::meaning::*;
use crate::support::package::tiny_package;
use crate::support::*;

const MODEL: &str = "local:tiny";

/// 八条记忆：前半是小词表里的词（向量各不一样），后面两个字是这一条独有的（关键词只撞得上它）。
const MEMORIES: [&str; 8] = [
    "我的猫 甲乙",
    "我的狗 丙丁",
    "喝茶 戊己",
    "hello world 庚辛",
    "猫狗 壬癸",
    "unaffable 子丑",
    "a world 寅卯",
    "喝茶的猫 辰巳",
];

/// 照下限表 `table`（没有的不交）接上照意思那一路、记八条、补齐向量。
async fn world(table: Option<&str>) -> (Home, Keeper, Using) {
    let home = Home::new();
    let setup = tiny_package(&home.scratch.0.join("packages").join("embed"));
    let vectors = Vectors::new(Some(Embedder::new(setup)), model_data(&home));
    assert!(home.memory.give_vectors(Arc::new(vectors)));
    if let Some(table) = table {
        let texts = RecallTexts::with(home.resources.path(), Thresholds::default())
            .expect("读得到")
            .searching(Thresholds::parse(table).expect("合写法"));
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

/// 搜一条：交回第一条的编号（没有的是空）。
fn first(keeper: &Keeper, query: &str, near: &Query) -> String {
    keeper
        .search(query, false, 1, Some(near))
        .expect("搜得了")
        .first()
        .map(|entry| entry.id.to_string())
        .unwrap_or_default()
}

/// 挑一条照意思排不进前四（她搜一条要四条）的：交回（编号、独有的两个字、问句的向量、它和问句多像）。
async fn behind(home: &Home, using: &Using) -> (String, String, Query, f32) {
    let vectors = home.memory.vectors().expect("接上了").clone();
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    for (n, text) in MEMORIES.iter().enumerate() {
        let tag = text.rsplit(' ').next().expect("有两个字").to_string();
        let near = vectors.query(using, &tag).await.expect("算得出");
        let top: Vec<String> = log
            .index()
            .nearest(MODEL, &near.vector, 4)
            .expect("找得了")
            .into_iter()
            .map(|hit| hit.key)
            .collect();
        let id = format!("m{}", n + 1);
        if !top.contains(&id) {
            let similar = log
                .index()
                .similar_to(MODEL, &near.vector, std::slice::from_ref(&id))
                .expect("算得了")[&id];
            return (id, tag, near, similar);
        }
    }
    panic!("八条里总有照意思排不进前四的");
}

fn table(model: &str, floor: f32) -> String {
    format!("[thresholds]\n\"{model}\" = {floor}\n")
}

#[tokio::test]
async fn a_keyword_hit_must_be_close_enough_in_meaning() {
    let (home, keeper, using) = world(None).await;
    let (id, tag, near, similar) = behind(&home, &using).await;
    assert_eq!(
        first(&keeper, &tag, &near),
        id,
        "没有下限表：关键词找到的排第一"
    );

    let above = (similar + 0.001).min(1.0);
    let (_, keeper, _) = world(Some(&table(MODEL, above))).await;
    let got = first(&keeper, &tag, &near);
    assert!(!got.is_empty(), "照意思那一路照样有");
    assert_ne!(got, id, "不到下限的、只有关键词找到的不要");

    let below = similar - 0.001;
    let (_, keeper, _) = world(Some(&table(MODEL, below))).await;
    assert_eq!(first(&keeper, &tag, &near), id, "过了下限的照旧");
}

#[tokio::test]
async fn a_model_not_in_the_table_or_without_the_vector_keeps_it() {
    let (home, _, using) = world(None).await;
    let (id, tag, near, _) = behind(&home, &using).await;

    let (_, keeper, _) = world(Some(&table("local:other", 1.0))).await;
    assert_eq!(first(&keeper, &tag, &near), id, "表里没有这个模型：不设");

    let other = Query {
        model: "local:other".to_string(),
        vector: near.vector.clone(),
    };
    assert_eq!(
        first(&keeper, &tag, &other),
        id,
        "表里有、这个模型还没有它的向量：照留"
    );
}

#[test]
fn the_shipped_table_reads() {
    let home = Home::new();
    let text = home
        .resources
        .memory_table("search")
        .expect("出厂的读得出来");
    let floors = Thresholds::parse(&text).expect("合写法");
    assert_eq!(
        floors.of("local:granite-embedding-97m-multilingual-r2"),
        Some(0.80)
    );
    assert_eq!(floors.of("local:bge-small-zh-v1.5"), None, "bge 不设");
    RecallTexts::load(&home.resources).expect("出厂的两张表读得出来");
}
