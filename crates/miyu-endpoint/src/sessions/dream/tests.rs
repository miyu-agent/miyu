//! 整理会话（施工 D-1 上，设计 41）：开一趟记 `session.created` 的 `dream`；一批一轮，请求只有这一批的原文（没有 system、
//! 工具、事实，同原来的一次性请求）；批间清空；主请求的 `model.called` 用途是 `memory`；叫停交回 `Stopped`。

use std::sync::Arc;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Purpose};
use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};
use miyu_session::{DreamBatch, DreamRoom, DreamSpec};
use miyu_store::log::read_events;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{resources, temp_root};

/// 照剧本 `script` 起来的核心，整理会话接上了；交回核心、数据根的目录、管理员。
fn core(script: &Script) -> (Arc<Core>, miyu_store::root::DataRoot, AccountId) {
    let (root, _) = temp_root("dream");
    let resources = resources();
    let tools =
        Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的工具")).expect("合写法");
    let admin = AccountId::parse("alice").expect("合写法");
    let core = Arc::new(Core::new(
        root.clone(),
        ResourceRoot::at(resources),
        Arc::new(script.clone()),
        tools,
        None,
        admin.clone(),
        "token".to_string(),
    ));
    assert!(core.give_dream_sessions(), "头一次接得上");
    assert!(!core.give_dream_sessions(), "接过的不再换");
    (core, root, admin)
}

/// 开一趟人格那一间、人叫的。
fn spec(admin: &AccountId) -> DreamSpec {
    DreamSpec {
        owner: admin.clone(),
        room: DreamRoom::Persona("engineer".to_string()),
        manual: true,
        from: None,
        model: None,
    }
}

fn id(session: &SessionId, n: u32) -> CommandId {
    CommandId::parse(&format!("{session}/dream/{n}")).expect("合写法")
}

/// 一条消息里的字。
fn words(message: &Message) -> String {
    let blocks = match message {
        Message::User { blocks } | Message::Assistant { blocks } => blocks,
        _ => return String::new(),
    };
    blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn each_batch_is_a_turn_of_its_own_with_nothing_but_the_batch() {
    let script = Script::new([
        Play::Says("第一批的回答"),
        Play::Says("第二批的回答"),
        Play::Says("第三批的回答"),
    ]);
    let (core, root, admin) = core(&script);
    let port = core.memory.dream_sessions().expect("接上了").clone();
    let session = port.open(spec(&admin)).await.expect("开得成");
    let first = port
        .batch(session.clone(), id(&session, 1), "第一批".to_string())
        .await
        .expect("跑得成");
    assert_eq!(first, DreamBatch::Answered("第一批的回答".to_string()));
    // 工具架换了一代：裸的不跟着重拼（重拼了就带回 system、工具；换来的策略下一轮才用上，所以看第三批）。
    core.tools
        .replace(|catalog| Ok::<_, ()>(catalog.clone()))
        .expect("换得了");
    let second = port
        .batch(session.clone(), id(&session, 2), "第二批".to_string())
        .await
        .expect("跑得成");
    assert_eq!(second, DreamBatch::Answered("第二批的回答".to_string()));
    let third = port
        .batch(session.clone(), id(&session, 3), "第三批".to_string())
        .await
        .expect("跑得成");
    assert_eq!(third, DreamBatch::Answered("第三批的回答".to_string()));

    let requests = script.requests();
    assert_eq!(requests.len(), 3, "一批一次请求");
    for ((_, request), batch) in requests.iter().zip(["第一批", "第二批", "第三批"]) {
        assert!(
            request.system.is_empty(),
            "没有 system：{:?}",
            request.system
        );
        assert!(request.tools.is_empty(), "没有工具");
        assert_eq!(
            request.messages.len(),
            1,
            "只有这一批，没有事实、上一批：{request:?}"
        );
        assert_eq!(words(&request.messages[0]), batch);
    }

    let dir = root.session_dir(&admin, &session);
    let events = read_events(&dir).expect("日志读得到");
    let Body::SessionCreated(created) = &events[0].body else {
        panic!("第一条是 session.created");
    };
    let dream = created.dream.as_ref().expect("带 dream");
    assert_eq!(dream.persona.as_deref(), Some("engineer"));
    assert!(dream.manual);
    assert_eq!(created.persona, None, "没有人格");
    let purposes: Vec<_> = events
        .iter()
        .filter_map(|event| match &event.body {
            Body::ModelCalled(called) => Some(called.purpose.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        purposes,
        vec![Some(Purpose::Memory); 3],
        "照会话记用量时分得出"
    );
    assert!(
        events
            .iter()
            .all(|event| !matches!(&event.body, Body::ContextInjected(_))),
        "回合开始不注入事实"
    );
}

#[tokio::test]
async fn stopping_interrupts_the_batch() {
    let script = Script::new([Play::Holds]);
    let (core, _root, admin) = core(&script);
    let port = core.memory.dream_sessions().expect("接上了").clone();
    let session = port.open(spec(&admin)).await.expect("开得成");
    let running = {
        let (port, session) = (port.clone(), session.clone());
        tokio::spawn(async move {
            port.batch(session.clone(), id(&session, 1), "一批".to_string())
                .await
        })
    };
    // 请求到了替身那里就停住了：等到那一次请求。
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while script.requests().is_empty() {
        assert!(tokio::time::Instant::now() < deadline, "请求没到");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let stop = CommandId::parse(&format!("{session}/dream/stop")).expect("合写法");
    port.stop(session, stop).await.expect("停得了");
    assert_eq!(running.await.expect("没 panic"), Ok(DreamBatch::Stopped));
}
