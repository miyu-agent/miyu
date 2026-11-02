//! 删人格带不带记忆（设计 55 第三节第 6 条，施工 FH-3）：删完三层里都没有这个编号了的，记忆日志跟着进回收处那一份的
//! `memory/`，派生的记忆库删掉；删的只是盖在出厂人格上的那一层的，记忆不动。

use std::path::Path;

use serde_json::{Value, json};

use crate::support::*;
use miyu_session::testkit::Script;

/// 样本人格 `engineer` 在下面那一层，连上照数据根读包的核心。
async fn below(home: &Home) -> Client {
    crate::support::packaged::sample_persona_below(home);
    let mut client = Client::connect(crate::support::packaged::core(home));
    client.hello().await;
    client
}

/// 人格 `id` 的记忆日志里放一段，连同派生的记忆库：交回日志目录。
fn remembered(home: &Home, id: &str) -> std::path::PathBuf {
    let log = home.root.memory_log(id);
    std::fs::create_dir_all(&log).expect("建得了");
    std::fs::write(log.join("000000000001.jsonl"), b"{\"seq\":1}\n").expect("写得进");
    let recall = home.root.index().join("recall");
    std::fs::create_dir_all(&recall).expect("建得了");
    std::fs::write(recall.join(format!("memory-{id}.db")), b"derived").expect("写得进");
    log
}

/// 回收处里编号 `id` 的那一份（只有一份）。
fn trashed(home: &Home, id: &str) -> std::path::PathBuf {
    let found: Vec<_> = std::fs::read_dir(home.root.trashed_personas())
        .expect("回收处建了")
        .map(|entry| entry.expect("读得出").path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&format!("{id}.")))
        })
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    found.into_iter().next().expect("有一份")
}

async fn delete(client: &mut Client, call: &str, id: &str) -> Value {
    client
        .call(call, "persona.delete", json!({"persona": id}))
        .await
}

fn file(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("000000000001.jsonl")).expect("在")
}

#[tokio::test]
async fn a_persona_gone_from_every_layer_takes_its_memory_along() {
    let home = Home::new();
    let mut client = below(&home).await;
    let made = client
        .call(
            "s1",
            "persona.set",
            json!({"prompts": {"persona": {"text": "Mine.\n"}}}),
        )
        .await;
    let id = made["result"]["persona"]
        .as_str()
        .expect("起了编号")
        .to_string();
    let log = remembered(&home, &id);
    let gone = delete(&mut client, "d1", &id).await;
    assert_eq!(gone["result"], json!({"remains": false}), "{gone}");
    assert!(!log.exists(), "记忆日志挪走了");
    let trashed = trashed(&home, &id);
    assert_eq!(
        file(&trashed.join("memory")),
        "{\"seq\":1}\n",
        "整份挪进回收处"
    );
    let derived = home
        .root
        .index()
        .join("recall")
        .join(format!("memory-{id}.db"));
    assert!(!derived.exists(), "派生的记忆库删了");
}

#[tokio::test]
async fn deleting_only_your_layer_over_a_shipped_one_keeps_the_memory() {
    let home = Home::new();
    let mut client = below(&home).await;
    let mine = client
        .call(
            "s1",
            "persona.set",
            json!({"persona": "engineer", "prompts": {"persona": {"text": "Mine too.\n"}}}),
        )
        .await;
    assert!(mine.get("result").is_some(), "{mine}");
    let log = remembered(&home, "engineer");
    let back = delete(&mut client, "d1", "engineer").await;
    assert_eq!(back["result"], json!({"remains": true}), "{back}");
    assert_eq!(file(&log), "{\"seq\":1}\n", "记忆还在原处");
    let trashed = trashed(&home, "engineer");
    assert!(!trashed.join("memory").exists(), "回收处里没有记忆");
    let derived = home.root.index().join("recall").join("memory-engineer.db");
    assert!(derived.exists(), "记忆库照旧");
}

#[tokio::test]
async fn a_persona_without_memory_is_deleted_as_before() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let made = client
        .call(
            "s1",
            "persona.set",
            json!({"prompts": {"persona": {"text": "Quiet.\n"}}}),
        )
        .await;
    let id = made["result"]["persona"]
        .as_str()
        .expect("起了编号")
        .to_string();
    let gone = delete(&mut client, "d1", &id).await;
    assert_eq!(gone["result"], json!({"remains": false}), "{gone}");
    assert!(!trashed(&home, &id).join("memory").exists());
}
