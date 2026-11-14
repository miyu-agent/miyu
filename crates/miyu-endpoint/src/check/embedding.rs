//! `check` 带的语义模型的状态（施工 R-25，`docs/blueprint/protocol.md` 的 `check` 第 5 条）：照这时的配置用哪个模型，
//! 用不上的为什么；后台在不在补；每个人格那一间的记忆、以前的对话，每个知识库，各有几条有这个模型的向量、一共几条。
//!
//! 只数：人格那一间照记忆日志的目录、回合库的文件在的数（开的是核心一直开着的那一份，不建新的），知识库照现在的索引数（不照文件
//! 补），都不起补。没接照意思那一路的核心（测试里）没有这一格。
//!
//! 另说这个模型自动联想不联想（施工 FX-4）：记忆的联想、知识库的自动想起都照模型的门槛表（`core/memory/recall.toml`、
//! `core/knowledge/recall.toml`），表里没有的模型（远程的、换了的）不联想（2026-10-10 项目主人定：没量过的不知道多像才算像）。
//! 换成远程的模型以后联想不出来、又没有任何提示，人以为坏了。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_recall::associate::Thresholds;
use miyu_store::recall::{RecallIndex, Room};
use miyu_store::resources::{ResourceRoot, SourceError};

use crate::Core;

/// 这一格：没接照意思那一路的没有。
pub(super) async fn embedding(core: &Core) -> Option<Value> {
    let vectors = Arc::clone(core.memory.vectors()?);
    let model = match vectors.state(&crate::memory::using(core)) {
        Ok(model) => model,
        Err(why) => return Some(json!({"model": null, "why": why})),
    };
    let (memory, knowledge) = (Arc::clone(&core.memory), Arc::clone(&core.knowledge));
    let counted = model.clone();
    let counts = tokio::task::spawn_blocking(move || {
        let count = |index: &RecallIndex| index.coverage(&counted).ok();
        let rooms = |personas: Vec<String>, open: &dyn Fn(&Room) -> Option<(u64, u64)>| {
            personas
                .into_iter()
                .filter_map(|persona| {
                    let (have, total) = open(&Room::persona(&persona))?;
                    Some(json!({"persona": persona, "have": have, "total": total}))
                })
                .collect::<Vec<Value>>()
        };
        let memories = rooms(memory.logs.personas().unwrap_or_default(), &|room| {
            count(memory.logs.open(room).ok()?.0.index())
        });
        let turns = rooms(memory.turns.personas().unwrap_or_default(), &|room| {
            count(&memory.turns.turns(room).0)
        });
        let bases: Vec<Value> = knowledge
            .ids()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| {
                let (have, total) = count(&knowledge.index_of(&id))?;
                let name = knowledge
                    .title(&id)
                    .map_or_else(|_| id.clone(), |(name, _)| name);
                Some(json!({"id": id, "name": name, "have": have, "total": total}))
            })
            .collect();
        (memories, turns, bases)
    })
    .await;
    let (memories, turns, bases) = counts.unwrap_or_default();
    let no_recall = unrecalled(&core.resources, &model);
    Some(json!({
        "model": model,
        "filling": vectors.filling(),
        "memory": memories,
        "turns": turns,
        "kb": bases,
        "no_recall": no_recall,
    }))
}

/// 模型 `model` 不自动联想的几样，照 `memory`、`kb` 的先后：门槛表里没有它的。表读不出来的同样不联想（会话那一头读不出来
/// 也不联想），照样列上。
fn unrecalled(resources: &ResourceRoot, model: &str) -> Vec<&'static str> {
    let recalls = |text: Result<String, SourceError>| {
        text.ok()
            .and_then(|text| Thresholds::parse(&text).ok())
            .is_some_and(|table| table.of(model).is_some())
    };
    let mut kinds = Vec::new();
    if !recalls(resources.memory_recall()) {
        kinds.push("memory");
    }
    if !recalls(resources.knowledge_table("recall")) {
        kinds.push("kb");
    }
    kinds
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use miyu_store::resources::ResourceRoot;

    use super::unrecalled;

    /// 出厂的门槛表：内置的模型两样都联想，表里没有的（远程的）两样都不（施工 FX-4）。
    #[test]
    fn only_a_measured_model_recalls_on_its_own() {
        let resources =
            ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
        assert!(
            unrecalled(&resources, "local:granite-embedding-97m-multilingual-r2").is_empty(),
            "内置的模型量过"
        );
        assert_eq!(
            unrecalled(&resources, "openai/text-embedding-3-small"),
            ["memory", "kb"]
        );
    }
}
