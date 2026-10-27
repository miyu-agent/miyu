//! 搜以前的对话的下限（施工 R-20，`docs/blueprint/recall.md` 第三条第 3 款）：回合库里直接放几段、配上手算的向量（四维、归一化），
//! 问句的向量也手给。只照意思找到的照 `core/memory/turns.toml` 的下限，表里没有的模型照 0.40；只照关键词找到的和问句不够像的
//! 照记忆搜那张 `core/memory/search.toml` 不要，还没补上向量的照留；出厂的表读得出。

use miyu_recall::associate::Thresholds;
use miyu_session::{Keeper, Listener, Query, RecallTexts};

use crate::support::meaning::persona;
use crate::support::*;

const MODEL: &str = "local:tiny";

/// 一个会话里的第 `turn` 轮的键。
fn key(turn: u64) -> String {
    format!("0192f3a0-1111-7abc-8def-001122334455/{turn}")
}

/// 归一化。
fn unit(vector: [f32; 4]) -> Vec<f32> {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    vector.iter().map(|x| x / norm).collect()
}

/// 问句的向量：和 `[1, 0, 0, 0]` 一个方向。
fn asking() -> Query {
    Query {
        model: MODEL.to_string(),
        vector: unit([1.0, 0.0, 0.0, 0.0]),
    }
}

/// 照两张表接上（`None` 的不交，照空表），回合库里放四段：
/// 1. 「团子吃鸡胸肉」和问句相似度 0.9；2. 「周末去爬山」0.6；3. 「甲乙丙丁」0.3（问句「甲乙」照关键词只撞得上它）；
/// 4. 「甲乙戊己」，没补上向量。
fn world(turns: Option<&str>, search: Option<&str>) -> (Home, Keeper) {
    let home = Home::new();
    let parse = |table: Option<&str>| {
        table.map_or_else(Thresholds::default, |table| {
            Thresholds::parse(table).expect("合写法")
        })
    };
    let texts = RecallTexts::with(home.resources.path(), Thresholds::default())
        .expect("读得到")
        .searching(parse(search))
        .searching_turns(parse(turns));
    seeded(home, texts, MODEL)
}

/// 照 `texts` 接上，往回合库里放那四段，向量记在模型 `model` 下。
fn seeded(home: Home, texts: RecallTexts, model: &str) -> (Home, Keeper) {
    assert!(home.memory.give_recall(texts));
    let (index, _) = home.recall.turns(&persona());
    let rows: [(&str, Option<[f32; 4]>); 4] = [
        ("团子吃鸡胸肉", Some([0.9, 0.436, 0.0, 0.0])),
        ("周末去爬山", Some([0.6, 0.0, 0.8, 0.0])),
        ("甲乙丙丁", Some([0.3, 0.0, 0.0, 0.954])),
        ("甲乙戊己", None),
    ];
    for (n, (text, vector)) in rows.into_iter().enumerate() {
        let key = key(u64::try_from(n).expect("放得下") + 1);
        index.put(&key, text, now()).expect("放得进");
        if let Some(vector) = vector {
            index
                .put_vector(&key, model, &unit(vector))
                .expect("放得进");
        }
    }
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    (home, keeper)
}

/// 搜 `query`，交回找到的是第几轮。
fn turns(keeper: &Keeper, query: &str, near: Option<&Query>) -> Vec<u64> {
    keeper
        .turns(query, 5, (None, None), near)
        .expect("搜得了")
        .into_iter()
        .map(|found| found.turn.started().get())
        .collect()
}

fn table(model: &str, floor: f32) -> String {
    format!("[thresholds]\n\"{model}\" = {floor}\n")
}

#[test]
fn only_close_enough_meaning_hits_come_in() {
    let near = asking();
    let (_home, keeper) = world(None, None);
    assert_eq!(
        turns(&keeper, "火星", Some(&near)),
        [1, 2],
        "空表照 0.40：两段都过"
    );
    let (_home, keeper) = world(Some(&table(MODEL, 0.84)), None);
    assert_eq!(turns(&keeper, "火星", Some(&near)), [1], "照表：0.6 的不要");
    let (_home, keeper) = world(Some(&table("local:other", 0.99)), None);
    assert_eq!(
        turns(&keeper, "火星", Some(&near)),
        [1, 2],
        "表里没有这个模型：照 0.40"
    );
}

#[test]
fn a_keyword_hit_must_be_close_enough_unless_it_has_no_vector() {
    let near = asking();
    let (_home, keeper) = world(Some(&table(MODEL, 0.84)), None);
    let found = turns(&keeper, "甲乙", Some(&near));
    assert!(
        found.contains(&3) && found.contains(&4),
        "没有关键词下限：{found:?}"
    );
    let (_home, keeper) = world(Some(&table(MODEL, 0.84)), Some(&table(MODEL, 0.80)));
    let found = turns(&keeper, "甲乙", Some(&near));
    assert!(
        !found.contains(&3),
        "只照关键词找到的、不够像的不要：{found:?}"
    );
    assert!(found.contains(&4), "还没补上向量的照留：{found:?}");
    assert_eq!(
        turns(&keeper, "甲乙", None),
        [3, 4],
        "没有问句的向量：照旧只走关键词"
    );
}

#[test]
fn the_shipped_table_reads() {
    let home = Home::new();
    let text = home
        .resources
        .memory_table("turns")
        .expect("出厂的读得出来");
    let floors = Thresholds::parse(&text).expect("合写法");
    assert_eq!(
        floors.of("local:granite-embedding-97m-multilingual-r2"),
        Some(0.84)
    );
    RecallTexts::load(&home.resources).expect("出厂的三张表读得出来");
}

/// 出厂的两张表接上就照它们办（granite：只照意思的 0.84、只照关键词的 0.80）。
#[test]
fn the_shipped_tables_apply() {
    const GRANITE: &str = "local:granite-embedding-97m-multilingual-r2";
    let home = Home::new();
    let texts = RecallTexts::load(&home.resources).expect("读得出来");
    let (_home, keeper) = seeded(home, texts, GRANITE);
    let near = Query {
        model: GRANITE.to_string(),
        vector: unit([1.0, 0.0, 0.0, 0.0]),
    };
    assert_eq!(turns(&keeper, "火星", Some(&near)), [1]);
    assert!(!turns(&keeper, "甲乙", Some(&near)).contains(&3));
}
