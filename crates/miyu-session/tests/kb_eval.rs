//! 知识库自动想起的测评集（施工 K-6 上，`docs/blueprint/knowledge/eval.md`）：虚构的世界观放进一个库，几十句人在聊天里会说的话，
//! 每句标着该想起哪几段；量几种挑法（话里提到了标题的名字、关键词搜得到、照意思像到门槛）在每种问句上带了多少、准不准，给 K-6
//! 下定怎么挑。规矩查一遍（CI 里也跑），量尺（`#[ignore]`）设了 `MIYU_EMBED_MODEL_DIR` 的连照意思那一路一起量。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;

use miyu_session::{Embedder, Turn, Using, Vectors};

use crate::support::meaning::{model_data, real_package};
use crate::support::{Home, alice_account};

/// 四种问句。
const KINDS: [&str; 4] = ["name", "paraphrase", "chat", "none"];

/// 照意思那一路量的几个门槛。
const LINES: [f32; 5] = [0.50, 0.55, 0.60, 0.65, 0.70];

/// 本机的那个模型的编号。
const MODEL: &str = "local:bge-small-zh-v1.5";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Set {
    query: Vec<Query>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    kind: String,
    text: String,
    expect: Vec<String>,
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/kb-eval")
}

fn shipped() -> Set {
    let text = std::fs::read_to_string(fixtures().join("queries.toml")).expect("读得到");
    toml_edit::de::from_str(&text).expect("合写法")
}

/// 测评集的每一段：「文档:标题」（标题路径的最后一段）对着它在索引里的键 `文档#第几块`，照知识库的切法算。
fn sections() -> BTreeMap<String, String> {
    let mut sections = BTreeMap::new();
    let world = fixtures().join("world");
    for entry in std::fs::read_dir(&world).expect("读得了") {
        let path = entry.expect("读得了").path();
        let name = path
            .file_name()
            .expect("有名字")
            .to_string_lossy()
            .into_owned();
        let text = std::fs::read_to_string(&path).expect("读得了");
        for (n, chunk) in miyu_recall::chunk::markdown(&text).iter().enumerate() {
            let heading = chunk.text.lines().next().unwrap_or_default();
            let leaf = heading
                .rsplit(" › ")
                .next()
                .unwrap_or_default()
                .trim_start_matches("# ");
            sections.insert(format!("{name}:{leaf}"), format!("{name}#{n}"));
        }
    }
    sections
}

#[test]
fn the_kb_eval_set_is_well_formed() {
    let set = shipped();
    let sections = sections();
    assert!(set.query.len() >= 40, "问句至少 40 句");
    let none = set
        .query
        .iter()
        .filter(|query| query.kind == "none")
        .count();
    assert!(none * 5 >= set.query.len(), "平常的话占两成以上");
    for kind in KINDS {
        assert!(
            set.query.iter().any(|query| query.kind == kind),
            "{kind} 至少一句"
        );
    }
    for query in &set.query {
        assert!(KINDS.contains(&query.kind.as_str()), "{}", query.text);
        assert!(!query.text.trim().is_empty());
        assert_eq!(
            query.kind == "none",
            query.expect.is_empty(),
            "{}",
            query.text
        );
        let unique: BTreeSet<&String> = query.expect.iter().collect();
        assert_eq!(unique.len(), query.expect.len(), "{}", query.text);
        for expected in &query.expect {
            assert!(
                sections.contains_key(expected),
                "{}：{expected} 不在",
                query.text
            );
        }
    }
}

/// 每句带了哪几段（键）。
type Picked = Vec<BTreeSet<String>>;

/// 照种类一行：句数、带了东西的占几成、带了的里该带的占几成、这一轮至少带对一段的占几成、平均带几段；最后一行 `all` 是有答案
/// 的几种合在一起。
fn table(title: &str, set: &Set, sections: &BTreeMap<String, String>, picked: &Picked) -> String {
    let mut out = format!(
        "{title}\n| 种类 | 句数 | 带了东西 | 带了的里该带的 | 这一轮带对了 | 平均带几段 |\n|---|---|---|---|---|---|\n"
    );
    let groups: Vec<(&str, Vec<usize>)> = KINDS
        .iter()
        .map(|kind| {
            (
                *kind,
                (0..set.query.len())
                    .filter(|&i| set.query[i].kind == *kind)
                    .collect(),
            )
        })
        .chain(std::iter::once((
            "all",
            (0..set.query.len())
                .filter(|&i| set.query[i].kind != "none")
                .collect(),
        )))
        .collect();
    for (kind, rows) in groups {
        let expected = |i: usize| -> BTreeSet<String> {
            set.query[i]
                .expect
                .iter()
                .map(|label| sections[label].clone())
                .collect()
        };
        let brought = rows.iter().filter(|&&i| !picked[i].is_empty()).count();
        let (right, total) = rows.iter().fold((0, 0), |(right, total), &i| {
            let good = picked[i].intersection(&expected(i)).count();
            (right + good, total + picked[i].len())
        });
        let hit = rows
            .iter()
            .filter(|&&i| picked[i].intersection(&expected(i)).next().is_some())
            .count();
        let share = |n: usize, of: usize| match of {
            0 => "—".to_string(),
            _ => format!("{:.0}%", n as f64 * 100.0 / of as f64),
        };
        let precise = share(right, total);
        let correct = match kind {
            "none" => "—".to_string(),
            _ => share(hit, rows.len()),
        };
        out.push_str(&format!(
            "| {kind} | {} | {} | {precise} | {correct} | {:.2} |\n",
            rows.len(),
            share(brought, rows.len()),
            total as f64 / rows.len().max(1) as f64
        ));
    }
    out
}

#[tokio::test]
#[ignore = "量尺：设了 MIYU_EMBED_MODEL_DIR 的连照意思那一路一起量"]
async fn measure_the_kb_eval_set() {
    let set = shipped();
    let sections = sections();
    let leaves: BTreeMap<String, String> = sections
        .iter()
        .map(|(label, key)| {
            (
                key.clone(),
                label.split_once(':').expect("有冒号").1.to_string(),
            )
        })
        .collect();
    let home = Home::new();
    let world = home.root.account_dir(&alice_account()).join("kb/world");
    std::fs::create_dir_all(&world).expect("建得了");
    for entry in std::fs::read_dir(fixtures().join("world")).expect("读得了") {
        let path = entry.expect("读得了").path();
        std::fs::copy(&path, world.join(path.file_name().expect("有名字"))).expect("拷得了");
    }
    let (bases, _) = home.knowledge.list(&alice_account()).expect("补得了");
    assert_eq!(bases[0].chunks, sections.len(), "一段一块");

    // 标题名：话里出现了这一段标题的最后一段（两个字以上）。
    let named: Picked = set
        .query
        .iter()
        .map(|query| {
            leaves
                .iter()
                .filter(|(_, leaf)| leaf.chars().count() >= 2 && query.text.contains(leaf.as_str()))
                .map(|(key, _)| key.clone())
                .collect()
        })
        .collect();
    println!("{}", table("标题名", &set, &sections, &named));
    // 关键词搜得到：只走关键词，前 3 块。
    let mut keyword = Picked::new();
    for query in &set.query {
        let (hits, _) = home
            .knowledge
            .search(&alice_account(), &query.text, None, 3, None)
            .expect("搜得了");
        keyword.push(
            hits.iter()
                .map(|hit| format!("{}#{}", hit.path, hit.chunk))
                .collect(),
        );
    }
    println!(
        "{}",
        table("关键词搜得到（前 3 块）", &set, &sections, &keyword)
    );

    let Some(files) = std::env::var_os("MIYU_EMBED_MODEL_DIR") else {
        println!("没设 MIYU_EMBED_MODEL_DIR：只量了标题名、关键词");
        return;
    };
    let setup = real_package(&home.scratch.0.join("packages/embed"), Path::new(&files));
    let vectors = Arc::new(Vectors::new(Some(Embedder::new(setup)), model_data(&home)));
    let using = Using {
        config: Arc::new(Turn::new(
            Default::default(),
            Arc::clone(&*home.configs.borrow()),
        )),
        owner: alice_account(),
    };
    let index = home.knowledge.index_of(&alice_account(), "world");
    vectors.fill_knowledge(&using, "kb alice world".to_string(), Arc::clone(&index));
    for _ in 0..1200 {
        if index.missing(MODEL, 0, 100).expect("读得了").is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        index.missing(MODEL, 0, 100).expect("读得了").is_empty(),
        "补齐了向量"
    );
    // 每句前 3 块的相似度。
    let mut near: Vec<Vec<(String, f32)>> = Vec::new();
    for query in &set.query {
        let asked = vectors.query(&using, &query.text).await.expect("算得出");
        let found = index.nearest(MODEL, &asked.vector, 3).expect("找得了");
        near.push(
            found
                .into_iter()
                .map(|hit| (hit.key, hit.similar))
                .collect(),
        );
    }
    for line in LINES {
        let meaning: Picked = near
            .iter()
            .map(|found| {
                found
                    .iter()
                    .filter(|(_, similar)| *similar >= line)
                    .map(|(key, _)| key.clone())
                    .collect()
            })
            .collect();
        println!(
            "{}",
            table(
                &format!("照意思（门槛 {line:.2}，前 3 块）"),
                &set,
                &sections,
                &meaning
            )
        );
        let both: Picked = meaning
            .iter()
            .zip(&named)
            .map(|(meaning, named)| meaning.union(named).cloned().collect())
            .collect();
        println!(
            "{}",
            table(
                &format!("标题名加照意思（门槛 {line:.2}）"),
                &set,
                &sections,
                &both
            )
        );
    }
    // 平常的话最像的那一段多像；有答案的，该带的那一段多像。
    let top = |i: usize| near[i].first().map_or(0.0, |(_, similar)| *similar);
    let mut quiet: Vec<f32> = (0..set.query.len())
        .filter(|&i| set.query[i].kind == "none")
        .map(top)
        .collect();
    quiet.sort_by(f32::total_cmp);
    println!("平常的话最像的那一段：{quiet:.3?}");
    for kind in ["name", "paraphrase", "chat"] {
        let mut right: Vec<f32> = (0..set.query.len())
            .filter(|&i| set.query[i].kind == kind)
            .map(|i| {
                let expected: BTreeSet<&String> = set.query[i]
                    .expect
                    .iter()
                    .map(|label| &sections[label])
                    .collect();
                near[i]
                    .iter()
                    .filter(|(key, _)| expected.contains(key))
                    .map(|(_, similar)| *similar)
                    .fold(0.0, f32::max)
            })
            .collect();
        right.sort_by(f32::total_cmp);
        println!("{kind} 该带的那一段多像（前 3 块里没有的是 0）：{right:.3?}");
    }
}
