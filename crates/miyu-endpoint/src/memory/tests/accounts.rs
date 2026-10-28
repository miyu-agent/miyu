//! 经协议记下的她自己（施工 Y-7 起照账号分间；施工 SU-4 中起只剩主人一间）：照人的写法记的还是一段经过。真的会话表、脚本模型。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::id::{AccountId, CommandId};
use miyu_session::testkit::{SAMPLE_PERSONA, Script, install_sample_persona};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{TempDir, resources, temp_root};

fn alice() -> AccountId {
    AccountId::parse("alice").expect("合写法")
}

/// 机主是 alice 的核心，照 `script` 答；装了样本人格（主人家目录）、整理会话。交回数据根的目录（丢掉时删掉，放在前头：核心先丢）
/// 和核心。
fn core(name: &str, script: &Script) -> (TempDir, Arc<Core>) {
    let (root, dir) = temp_root(name);
    install_sample_persona(&root);
    let resources = resources();
    let tools =
        Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的工具")).expect("合写法");
    let core = Arc::new(Core::new(
        root,
        ResourceRoot::at(resources),
        Arc::new(script.clone()),
        tools,
        None,
        alice(),
        "token".to_string(),
    ));
    assert!(core.give_dream_sessions());
    (dir, core)
}

/// 照账号 `who` 调一条 `memory.*`。
async fn call(core: &Arc<Core>, who: &AccountId, id: &str, method: &str, params: Value) -> Value {
    let id = CommandId::parse(id).expect("合写法");
    super::super::call(core, who, method, &id, &params)
        .await
        .unwrap_or_else(|refusal| panic!("{method} 办得了：{}", refusal.reason))
}

/// 经协议记一个认识的人、`who` 是她自己的名字（施工 E-1 修）：照人格找那一间时记下她叫什么，这一条记成关于她自己的事。
#[tokio::test]
async fn remembering_her_as_a_person_by_protocol_keeps_it_as_an_episode() {
    let (_dir, core) = core("self-person", &Script::new([]).once(Vec::<&str>::new()));
    call(
        &core,
        &alice(),
        "p1",
        "memory.remember",
        json!({"persona": SAMPLE_PERSONA, "class": "person", "who": "software engineer", "text": "她英语不好"}),
    )
    .await;
    let listed = call(
        &core,
        &alice(),
        "p2",
        "memory.list",
        json!({"persona": SAMPLE_PERSONA}),
    )
    .await;
    let memory = &listed["memories"][0];
    assert_eq!(memory["class"], "episode", "{listed}");
    assert!(memory.get("who").is_none_or(Value::is_null), "{listed}");
}
