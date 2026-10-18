//! 知识库自动想起的测评集（施工 K-6 上，`docs/blueprint/knowledge/eval.md`）：虚构的世界观放进一个库，几十句人在聊天里会说的话，
//! 每句标着该想起哪几段；量几种挑法（话里提到了标题的名字、关键词搜得到、照意思像到门槛）在每种问句上带了多少、准不准，给 K-6
//! 下定怎么挑。规矩查一遍（CI 里也跑），量尺（`#[ignore]`）设了 `MIYU_EMBED_MODEL_DIR` 的连照意思那一路一起量。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use serde::Deserialize;

use miyu_session::{Embedder, Turn, Using, Vectors};

use crate::support::meaning::{model_data, real_package, shipped_model};
use crate::support::{Home, alice_account};

/// 四种问句。
const KINDS: [&str; 4] = ["name", "paraphrase", "chat", "none"];

/// 照意思那一路量的几个门槛：granite 的尺度（施工 R-5 七补；bge 时量的是 0.50 到 0.70）。
const LINES: [f32; 6] = [0.80, 0.82, 0.84, 0.86, 0.88, 0.90];

/// `kb_search` 只照意思找到的那一路量的几个下限：同上（bge 时是 0.40 到 0.65）。
const FLOORS: [f32; 6] = [0.76, 0.78, 0.80, 0.81, 0.82, 0.84];

/// 本机的那个模型的编号：照出厂的模型清单。
static MODEL: LazyLock<String> = LazyLock::new(shipped_model);

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

/// 测评集的世界观放进 alice 的库 `world`，照文件补好（一段一块）。
fn world(sections: &BTreeMap<String, String>) -> Home {
    let home = Home::new();
    let world = home.root.account_dir(&alice_account()).join("kb/world");
    std::fs::create_dir_all(&world).expect("建得了");
    for entry in std::fs::read_dir(fixtures().join("world")).expect("读得了") {
        let path = entry.expect("读得了").path();
        std::fs::copy(&path, world.join(path.file_name().expect("有名字"))).expect("拷得了");
    }
    let (bases, _) = home.knowledge.list(&alice_account()).expect("补得了");
    assert_eq!(bases[0].chunks, sections.len(), "一段一块");
    home
}

/// 设了 `MIYU_EMBED_MODEL_DIR` 的：接上出厂的真模型，补齐库 `world` 的向量，交回算问句向量要的两样；没设的没有。
async fn real(home: &Home) -> Option<(Arc<Vectors>, Using)> {
    let files = std::env::var_os("MIYU_EMBED_MODEL_DIR")?;
    let setup = real_package(&home.scratch.0.join("packages/embed"), Path::new(&files));
    let vectors = Arc::new(Vectors::new(Some(Embedder::new(setup)), model_data(home)));
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
        if index.missing(&MODEL, 0, 100).expect("读得了").is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        index.missing(&MODEL, 0, 100).expect("读得了").is_empty(),
        "补齐了向量"
    );
    Some((vectors, using))
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
    let home = world(&sections);

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

    let Some((vectors, using)) = real(&home).await else {
        println!("没设 MIYU_EMBED_MODEL_DIR：只量了标题名、关键词");
        return;
    };
    let index = home.knowledge.index_of(&alice_account(), "world");
    // 每句前 3 块的相似度。
    let mut near: Vec<Vec<(String, f32)>> = Vec::new();
    for query in &set.query {
        let asked = vectors.query(&using, &query.text).await.expect("算得出");
        let found = index.nearest(&MODEL, &asked.vector, 3).expect("找得了");
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

/// `kb_search` 只照意思找到的那一路的下限（施工 K-2 修，验收手册待定 11）：同她的工具，每句关键词、照意思各找 20 块，只有照意思
/// 找到的过下限才算，两路照名次合，取前 5 块；几个下限各打一张表。平常的话（`none`）带了东西的就是乱带。
#[tokio::test]
#[ignore = "量尺：要设 MIYU_EMBED_MODEL_DIR"]
async fn measure_the_search_floor() {
    let set = shipped();
    let sections = sections();
    let home = world(&sections);
    let Some((vectors, using)) = real(&home).await else {
        println!("没设 MIYU_EMBED_MODEL_DIR：量不了");
        return;
    };
    let index = home.knowledge.index_of(&alice_account(), "world");
    let mut asked = Vec::new();
    for query in &set.query {
        let words: Vec<String> = index
            .search(&query.text, 20)
            .expect("搜得了")
            .into_iter()
            .map(|hit| hit.key)
            .collect();
        let near = vectors.query(&using, &query.text).await.expect("算得出");
        let close: Vec<(String, f32)> = index
            .nearest(&MODEL, &near.vector, 20)
            .expect("找得了")
            .into_iter()
            .map(|hit| (hit.key, hit.similar))
            .collect();
        asked.push((words, close));
    }
    for floor in FLOORS {
        let picked: Picked = asked
            .iter()
            .map(|(words, close)| {
                let close: Vec<(String, f32)> = close
                    .iter()
                    .filter(|(key, similar)| *similar >= floor || words.contains(key))
                    .cloned()
                    .collect();
                miyu_recall::fuse(words, &close)
                    .into_iter()
                    .take(5)
                    .collect()
            })
            .collect();
        println!(
            "{}",
            table(
                &format!("kb_search，下限 {floor:.2}，前 5 块"),
                &set,
                &sections,
                &picked
            )
        );
    }
    let picked: Picked = asked
        .iter()
        .map(|(words, _)| words.iter().take(5).cloned().collect())
        .collect();
    println!(
        "{}",
        table("kb_search，只走关键词，前 5 块", &set, &sections, &picked)
    );
    // 测评集里换了说法的多半还有一两个字对得上；这几句一个字都对不上，只能照意思找到：该找到的那一段多像。
    for (text, label) in [
        ("雷雨天谁最害怕", "people.md:艾琳"),
        ("喷火的大怪兽住在哪", "people.md:赤鳞"),
        ("这里的钞票长什么样", "lore.md:银鳞币"),
        ("哪个节日要放飞亮光", "history.md:灯笼节"),
    ] {
        let near = vectors.query(&using, text).await.expect("算得出");
        let found = index.nearest(&MODEL, &near.vector, 20).expect("找得了");
        let rank = found.iter().position(|hit| hit.key == sections[label]);
        let similar = rank.map(|at| found[at].similar);
        println!("{text} → {label}：第 {rank:?} 像，{similar:.3?}");
    }
}
