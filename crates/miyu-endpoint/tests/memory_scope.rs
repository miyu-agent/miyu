//! 记忆的范围（施工 R-3 下，`docs/blueprint/memory.md`「范围」，`protocol.md` 的 `session.create`）：真核心走一遍。`memory`
//! 两种照写的记进快照，写错的参数不对、什么都不造；不写的跟着人格。「只在这个会话里」去掉了（施工 FH-1，设计 55 第六节第 3
//! 条）：写 `session` 的照写错的回，人格的 `persona.toml` 里不再有 `[memory]`。

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_policy::Snapshot;
use miyu_policy::memory::MemoryScope;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;

use crate::support::*;

/// 会话 `session` 快照里记忆的范围。
fn scope(home: &Home, session: &str) -> MemoryScope {
    let log = home.log(session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话");
    };
    let bytes = Blobs::new(home.root.blobs())
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂").memory_scope()
}

async fn create(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "session.create", params).await
}

#[tokio::test]
async fn session_create_records_the_scope_and_refuses_a_wrong_one() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    for (id, memory, expected) in [
        ("c1", json!("persona"), MemoryScope::Persona),
        ("c3", json!("off"), MemoryScope::Off),
        ("c4", Value::Null, MemoryScope::Persona),
    ] {
        let reply = create(
            &mut client,
            id,
            json!({"cwd": "~", "persona": "engineer", "memory": memory}),
        )
        .await;
        let session = reply["result"]["session"].as_str().expect("造了");
        assert_eq!(scope(&home, session), expected, "{memory}");
    }
    for (id, memory) in [
        ("c2", json!("session")),
        ("c5", json!("Off")),
        ("c6", json!("everywhere")),
        ("c7", json!(3)),
    ] {
        let reply = create(&mut client, id, json!({"cwd": "~", "memory": memory})).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{memory}：{reply}");
    }
    let listed = client.call("l1", "session.list", json!({})).await;
    assert_eq!(
        listed["result"]["sessions"].as_array().map(Vec::len),
        Some(3),
        "写错的什么都没造"
    );
}

/// 人格不再管范围（施工 FH-1）：`persona.toml` 写了 `[memory]` 的照不认识的表报，会话不造；不写的跟着人格。
#[tokio::test]
async fn a_memory_table_in_the_persona_file_is_refused() {
    let home = Home::new();
    home.write(
        "personas/engineer/persona.toml",
        "[memory]\nscope = \"persona\"\n",
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = create(
        &mut client,
        "c1",
        json!({"cwd": "~", "persona": "engineer"}),
    )
    .await;
    assert_eq!(reason(&reply), Some("persona_invalid"), "{reply}");
    assert!(reply.to_string().contains("unknown table"), "{reply}");
}

/// 无人格的会话也有记忆（施工 P-12，2026-10-17 项目主人定，改掉 P-4 上的「无人格记忆不生效」）：范围照写的；不写的跟着人格，
/// 落在无人格自己那一间。不写人格、没设默认人格的就是无人格。
#[tokio::test]
async fn without_a_persona_memory_follows_what_is_asked() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    for (id, params, expected) in [
        ("c1", json!({"cwd": "~"}), MemoryScope::Persona),
        (
            "c2",
            json!({"cwd": "~", "persona": null, "memory": "persona"}),
            MemoryScope::Persona,
        ),
        ("c4", json!({"cwd": "~", "memory": "off"}), MemoryScope::Off),
    ] {
        let reply = create(&mut client, id, params.clone()).await;
        let session = reply["result"]["session"].as_str().expect("造了");
        assert_eq!(scope(&home, session), expected, "{params}");
    }
}
