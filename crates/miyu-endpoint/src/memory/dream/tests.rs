//! 现在就整理跑在整理会话里（施工 D-2，设计 41）：真的会话表、脚本模型。`memory.dream` 的回应带这一趟的会话，那个会话的日志
//! 里是记忆模块发的一批、用途 `memory` 的请求、答的那一段；记忆日志的 `merging` 带同一个会话。

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::event::{Body, Purpose};
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_recall::{MemoryEvent, from_event};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Keeper, Listener};
use miyu_store::log::read_events;
use miyu_store::recall::{NO_PERSONA, Room};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{resources, temp_root};

#[tokio::test]
async fn a_dream_runs_in_a_dream_session_and_names_it() {
    // 一次性入口排了零个回答：走一次性入口的会没有模型可用，测得出走的是整理会话。
    let script =
        Script::new([Play::Says(r#"{"summary": "用户养了一只猫。"}"#)]).once(Vec::<&str>::new());
    let (root, _dir) = temp_root("dream-session");
    let resources = resources();
    let tools =
        Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的工具")).expect("合写法");
    let core = Arc::new(Core::new(
        root.clone(),
        ResourceRoot::at(resources),
        Arc::new(script.clone()),
        tools,
        None,
        "token".to_string(),
    ));
    assert!(core.give_dream_sessions());
    let at = json!({"persona": NO_PERSONA});
    let remember = json!({"persona": NO_PERSONA, "class": "user", "text": "用户养了一只猫"});
    let id = CommandId::parse("c1").expect("合写法");
    super::super::call(&core, "memory.remember", &id, &remember)
        .await
        .expect("记得下");

    let answer = super::call(&core, &at).await.expect("整理了");
    let session =
        SessionId::parse(answer["session"].as_str().expect("回应带会话")).expect("合写法");
    assert_eq!(answer["summary"], json!(true), "{answer}");

    let events = read_events(&root.session_dir(&session)).expect("日志读得到");
    let Body::SessionCreated(created) = &events[0].body else {
        panic!("第一条是 session.created");
    };
    let dream = created.dream.as_ref().expect("是整理会话");
    assert!(dream.manual, "人叫的");
    let asked = events
        .iter()
        .find(|event| matches!(event.body, Body::MessageUser(_)))
        .expect("发了一批");
    assert!(
        matches!(&asked.by, By::Module(module) if module.id.as_str() == "memory"),
        "记忆模块发的：{:?}",
        asked.by
    );
    assert!(
        events.iter().any(|event| matches!(
            &event.body,
            Body::ModelCalled(called) if called.purpose == Some(Purpose::Memory)
        )),
        "用途是记忆"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event.body, Body::MessageAssistant(_))),
        "答的那一段在会话里"
    );

    let keeper = Keeper::new(
        &core.memory,
        Room::persona(NO_PERSONA),
        Listener::Owner(crate::sessions::person()),
    );
    let merging = keeper
        .follow(Some(0), 64)
        .expect("跟得上")
        .filled
        .iter()
        .filter_map(|event| from_event(event)?.ok())
        .find_map(|event| match event {
            MemoryEvent::Merging(merging) => Some(merging),
            _ => None,
        })
        .expect("记了开始合");
    assert_eq!(merging.session, Some(session));
    // 落完以后补关键词（施工 R-15 下）照旧走一次性入口；合并那一批不走。
    assert!(
        script
            .asked()
            .iter()
            .all(|ask| !format!("{:?}", ask.messages).contains("Below are your memories")),
        "合并没走一次性入口"
    );
}
