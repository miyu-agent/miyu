//! `check` 带的语义模型的状态（施工 R-25，`docs/blueprint/protocol.md` 的 `check` 第 5 条）：照管理员这时的配置用哪个模型，
//! 用不上的为什么；后台在不在补；管理员名下每个人格那一间的记忆、以前的对话，每个知识库，各有几条有这个模型的向量、一共几条。
//!
//! 只数：人格那一间照记忆日志的目录、回合库的文件在的数（开的是核心一直开着的那一份，不建新的），知识库照现在的索引数（不照文件
//! 补），都不起补。没接照意思那一路的核心（测试里）没有这一格。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_store::recall::{RecallIndex, Room};

use crate::Core;

/// 这一格：没接照意思那一路的没有。
pub(super) async fn embedding(core: &Core) -> Option<Value> {
    let vectors = Arc::clone(core.memory.vectors()?);
    let model = match vectors.state(&crate::memory::using(core)) {
        Ok(model) => model,
        Err(why) => return Some(json!({"model": null, "why": why})),
    };
    let (memory, knowledge, admin) = (
        Arc::clone(&core.memory),
        Arc::clone(&core.knowledge),
        core.admin.clone(),
    );
    let counted = model.clone();
    let counts = tokio::task::spawn_blocking(move || {
        let count = |index: &RecallIndex| index.coverage(&counted).ok();
        let rooms = |personas: Vec<String>, open: &dyn Fn(&Room) -> Option<(u64, u64)>| {
            personas
                .into_iter()
                .filter_map(|persona| {
                    let (have, total) = open(&Room::persona(&admin, &persona))?;
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
                let (have, total) = count(&knowledge.index_of(&admin, &id))?;
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
    Some(json!({
        "model": model,
        "filling": vectors.filling(),
        "memory": memories,
        "turns": turns,
        "kb": bases,
    }))
}
