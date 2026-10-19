//! 量尺：以前的对话（回合索引）搜的下限（施工 R-20，`docs/blueprint/recall.md` 第三条第 3 款）。读仓库外的一份回合和问句
//! （`MIYU_TURNS_EVAL_IN`：`{"model", "turns": [{"key", "text", "vector"}…], "questions": [{"id", "text", "vector"}…]}`，向量是
//! 那个模型算好的），照真的检索库放进去，每句照关键词、照意思各找前 40 条，带上和问句的相似度，写到 `MIYU_TURNS_EVAL_OUT`；
//! 判哪几条对、比几种下限在仓库外做。没设的什么都不量。数据是人的，不进仓库。

use miyu_store::recall::RecallIndex;
use serde::Deserialize;
use serde_json::json;

use crate::support::*;

#[derive(Deserialize)]
struct Input {
    model: String,
    turns: Vec<Item>,
    questions: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    #[serde(alias = "key")]
    id: String,
    text: String,
    vector: Vec<f32>,
}

/// 每一路找几条。
const WIDE: usize = 40;

#[test]
#[ignore = "量尺：要设 MIYU_TURNS_EVAL_IN、MIYU_TURNS_EVAL_OUT"]
fn measure_the_turns_floor() {
    let (Ok(input), Ok(output)) = (
        std::env::var("MIYU_TURNS_EVAL_IN"),
        std::env::var("MIYU_TURNS_EVAL_OUT"),
    ) else {
        println!("没设 MIYU_TURNS_EVAL_IN、MIYU_TURNS_EVAL_OUT：不量");
        return;
    };
    let input: Input =
        serde_json::from_str(&std::fs::read_to_string(input).expect("读得到")).expect("合写法");
    let scratch = Scratch::new("turns-eval");
    let place = root_in(&scratch)
        .index(&admin())
        .join("recall")
        .join("turns.db");
    let (index, _) = RecallIndex::open(&place);
    for turn in &input.turns {
        index.put(&turn.id, &turn.text, at(1)).expect("放得进");
        index
            .put_vector(&turn.id, &input.model, &turn.vector)
            .expect("放得进");
    }
    let mut found = Vec::new();
    for question in &input.questions {
        let words: Vec<String> = index
            .search(&question.text, WIDE)
            .expect("搜得了")
            .into_iter()
            .map(|hit| hit.key)
            .collect();
        let similar = index
            .similar_to(&input.model, &question.vector, &words)
            .expect("算得了");
        let near: Vec<(String, f32)> = index
            .nearest(&input.model, &question.vector, WIDE)
            .expect("找得了")
            .into_iter()
            .map(|near| (near.key, near.similar))
            .collect();
        found.push(json!({
            "id": question.id,
            "words": words.iter().map(|key| json!([key, similar.get(key)])).collect::<Vec<_>>(),
            "near": near,
        }));
    }
    std::fs::write(output, serde_json::to_string(&found).expect("写得成")).expect("写得进");
    println!("{} 句", found.len());
}
