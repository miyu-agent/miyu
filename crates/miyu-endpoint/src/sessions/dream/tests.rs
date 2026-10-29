//! 整理会话（施工 D-1 上，设计 41）：开一趟记 `session.created` 的 `dream`；一批一轮，请求只有这一批的原文（没有 system、
//! 工具、事实，同原来的一次性请求）；批间清空；主请求的 `model.called` 用途是 `memory`；叫停交回 `Stopped`。D-1 下：正文边流边
//! 交（思考不交）；`session.list` 带 `kind`、照 `dream` 挑，她的 `sessions` 工具不列；跑完一趟只留这一间最近几趟。DR-1：收尾
//! 交了结果的记一条 `dream.ended`，列表照它写 `dream.result`、推一声。

use std::sync::Arc;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Purpose};
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};
use miyu_session::{DreamBatch, DreamResult, DreamRoom, DreamSpec, Watched};
use miyu_store::log::read_events;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use super::memory_module;
use crate::Core;
use crate::test_support::{TempDir, resources, temp_root};

/// 照剧本 `script` 起来的核心，整理会话接上了；交回数据根的目录（丢掉时删掉，放在前头：核心先丢）、核心、数据根。
fn core(script: &Script) -> (TempDir, Arc<Core>, miyu_store::root::DataRoot) {
    let (root, dir) = temp_root("dream");
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
    assert!(core.give_dream_sessions(), "头一次接得上");
    assert!(!core.give_dream_sessions(), "接过的不再换");
    (dir, core, root)
}

/// 开一趟人格那一间、人叫的。
fn spec() -> DreamSpec {
    spec_of("engineer")
}

/// 开一趟人格 `persona` 那一间、人叫的。
fn spec_of(persona: &str) -> DreamSpec {
    DreamSpec {
        room: DreamRoom::Persona(persona.to_string()),
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
    let (_dir, core, root) = core(&script);
    let port = core.memory.dream_sessions().expect("接上了").clone();
    let session = port.open(spec()).await.expect("开得成");
    let first = port
        .batch(
            session.clone(),
            id(&session, 1),
            "第一批".to_string(),
            &mut |_| {},
        )
        .await
        .expect("跑得成");
    assert_eq!(first, DreamBatch::Answered("第一批的回答".to_string()));
    // 工具架换了一代：裸的不跟着重拼（重拼了就带回 system、工具；换来的策略下一轮才用上，所以看第三批）。
    core.tools
        .replace(|catalog| Ok::<_, ()>(catalog.clone()))
        .expect("换得了");
    let second = port
        .batch(
            session.clone(),
            id(&session, 2),
            "第二批".to_string(),
            &mut |_| {},
        )
        .await
        .expect("跑得成");
    assert_eq!(second, DreamBatch::Answered("第二批的回答".to_string()));
    let third = port
        .batch(
            session.clone(),
            id(&session, 3),
            "第三批".to_string(),
            &mut |_| {},
        )
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

    let dir = root.session_dir(&session);
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
    let (_dir, core, _root) = core(&script);
    let port = core.memory.dream_sessions().expect("接上了").clone();
    let session = port.open(spec()).await.expect("开得成");
    let running = {
        let (port, session) = (port.clone(), session.clone());
        tokio::spawn(async move {
            port.batch(
                session.clone(),
                id(&session, 1),
                "一批".to_string(),
                &mut |_| {},
            )
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

#[tokio::test]
async fn the_answer_streams_to_the_watch_without_the_thinking() {
    let script = Script::new([Play::Thinks {
        thinking: "先想想",
        text: "回答的原文",
    }]);
    let (_dir, core, _root) = core(&script);
    let port = core.memory.dream_sessions().expect("接上了").clone();
    let session = port.open(spec()).await.expect("开得成");
    let mut heard = String::new();
    let answered = port
        .batch(
            session.clone(),
            id(&session, 1),
            "一批".to_string(),
            &mut |watched| {
                if let Watched::Text(text) = watched {
                    heard.push_str(text);
                }
            },
        )
        .await
        .expect("跑得成");
    assert_eq!(answered, DreamBatch::Answered("回答的原文".to_string()));
    assert_eq!(heard, "回答的原文", "边流边交的就是回答，思考不交");
}

#[tokio::test]
async fn dream_sessions_are_listed_with_their_kind_but_not_to_her() {
    let (_dir, core, _root) = core(&Script::new([]));
    let port = core.memory.dream_sessions().expect("接上了").clone();
    let session = port.open(spec()).await.expect("开得成");
    let all = crate::list::list(&core, false, None, None)
        .await
        .expect("列得出");
    let item = all
        .iter()
        .find(|item| item["session"] == session.as_str())
        .expect("照常列");
    assert_eq!(item["kind"], "dream");
    assert_eq!(item["dream"]["persona"], "engineer");
    assert_eq!(item["dream"]["manual"], true);
    let only = crate::list::list(&core, false, Some(true), None)
        .await
        .expect("列得出");
    assert_eq!(only.len(), 1, "只要整理会话：{only:?}");
    let none = crate::list::list(&core, false, Some(false), None)
        .await
        .expect("列得出");
    assert!(
        none.iter().all(|item| item["session"] != session.as_str()),
        "不要整理会话：{none:?}"
    );
    let hers = crate::spawn::port(&core)
        .sessions(miyu_tool::Stop::default())
        .await
        .expect("列得出");
    assert!(
        hers.iter().all(|main| main.id != session),
        "她的 sessions 不列"
    );
}

#[tokio::test]
async fn closing_keeps_only_the_newest_of_the_room() {
    let (_dir, core, root) = core(&Script::new([]));
    let port = core.memory.dream_sessions().expect("接上了").clone();
    let mut engineer = Vec::new();
    for _ in 0..3 {
        engineer.push(port.open(spec()).await.expect("开得成"));
    }
    let other = port.open(spec_of("other")).await.expect("开得成");
    let exists = |session: &SessionId| root.session_dir(session).exists();

    assert_eq!(port.close(engineer[2].clone(), 2, None).await, Ok(1));
    assert!(!exists(&engineer[0]), "最旧的删了");
    assert!(exists(&engineer[1]) && exists(&engineer[2]));
    assert!(exists(&other), "别间的不动");

    assert_eq!(
        port.close(engineer[2].clone(), 0, None).await,
        Ok(2),
        "0 连它一起删"
    );
    assert!(!exists(&engineer[1]) && !exists(&engineer[2]));
    assert!(exists(&other));
}

/// 列表里会话 `session` 那一项。
async fn listed(core: &Arc<Core>, session: &SessionId) -> serde_json::Value {
    crate::list::list(core, false, None, None)
        .await
        .expect("列得出")
        .into_iter()
        .find(|item| item["session"] == session.as_str())
        .expect("照常列")
}

#[tokio::test]
async fn closing_with_a_result_records_it_and_lists_it() {
    let (_dir, core, root) = core(&Script::new([]));
    let port = core.memory.dream_sessions().expect("接上了").clone();
    let done = port.open(spec()).await.expect("开得成");
    let stopped = port.open(spec()).await.expect("开得成");
    let bare = port.open(spec()).await.expect("开得成");
    let mut pushes = core.listing.subscribe();
    let result = DreamResult {
        given: 3,
        revised: 1,
        retired: 2,
        summary: true,
        stopped: false,
    };
    assert_eq!(port.close(done.clone(), 5, Some(result)).await, Ok(0));
    let halted = DreamResult {
        given: 1,
        revised: 0,
        retired: 0,
        summary: false,
        stopped: true,
    };
    assert_eq!(port.close(stopped.clone(), 5, Some(halted)).await, Ok(0));
    assert_eq!(port.close(bare.clone(), 5, None).await, Ok(0));

    let events = read_events(&root.session_dir(&done)).expect("读得出");
    let last = events.last().expect("有事件");
    assert!(
        matches!(&last.body, Body::DreamEnded(ended) if *ended == result),
        "末尾记一条：{last:?}"
    );
    assert_eq!(
        last.by,
        memory_module().expect("合写法"),
        "以记忆模块的名义"
    );
    assert_eq!(last.turn, None, "不带回合");

    let item = listed(&core, &done).await;
    assert_eq!(
        item["dream"]["result"],
        serde_json::json!({"given": 3, "revised": 1, "retired": 2, "summary": true}),
        "没叫停的不写 stopped：{item}"
    );
    let item = listed(&core, &stopped).await;
    assert_eq!(item["dream"]["result"]["stopped"], true, "{item}");
    let item = listed(&core, &bare).await;
    assert!(
        item["dream"].get("result").is_none(),
        "没交结果的不写：{item}"
    );
    assert_eq!(item["dream"]["persona"], "engineer");

    // 会话列表的推送照它报一声（`sessions.changed`）。
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let change = pushes.recv().await.expect("推送还在");
            if change.line.contains(done.as_str()) && change.line.contains("\"result\"") {
                return change;
            }
        }
    })
    .await
    .expect("推了带结果的那一项");
}
