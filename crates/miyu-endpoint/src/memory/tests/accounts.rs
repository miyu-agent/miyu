//! 记忆按账号分层（施工 Y-7，设计 06 U21：记忆跟着用户走，用户和用户的记忆不串）：成员的本机会话也有记忆，记在他自己那一间；
//! 用系统区的人格（别人分享的）也记在用的人那里；`memory.*` 照替谁干活的账号找那一间，机主和成员互相看不到。真的会话表、
//! 脚本模型；成员连不进协议（成员闸，Y-2 下下），这里直接照账号调。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_kernel::origin::{By, Person};
use miyu_kernel::session::Command;
use miyu_session::testkit::{Play, SAMPLE_PERSONA, Script, install_sample_persona};
use miyu_store::log::read_events;
use miyu_store::recall::{NO_PERSONA, Room};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::sessions::Opening;
use crate::test_support::{resources, temp_root};

fn alice() -> AccountId {
    AccountId::parse("alice").expect("合写法")
}

fn bob() -> AccountId {
    AccountId::parse("bob").expect("合写法")
}

/// 机主是 alice 的核心，照 `script` 答；装了样本人格（系统区，谁都用得到）、整理会话。
fn core(name: &str, script: &Script) -> Arc<Core> {
    let (root, _) = temp_root(name);
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
    core
}

/// 照账号 `who` 调一条 `memory.*`。
async fn call(core: &Arc<Core>, who: &AccountId, id: &str, method: &str, params: Value) -> Value {
    let id = CommandId::parse(id).expect("合写法");
    super::super::call(core, who, method, &id, &params)
        .await
        .unwrap_or_else(|refusal| panic!("{method} 办得了：{}", refusal.reason))
}

/// `who` 那一间（样本人格）列得出的正文。
async fn texts(core: &Arc<Core>, who: &AccountId, id: &str) -> Vec<String> {
    let listed = call(
        core,
        who,
        id,
        "memory.list",
        json!({"persona": SAMPLE_PERSONA}),
    )
    .await;
    listed["memories"]
        .as_array()
        .expect("有 memories")
        .iter()
        .filter_map(|memory| memory["text"].as_str().map(str::to_string))
        .collect()
}

/// 账号 `owner` 用样本人格开一个本机会话，交回编号。
async fn open(core: &Arc<Core>, owner: &AccountId, command: &str) -> SessionId {
    let workspace = core.root.workspace(owner);
    std::fs::create_dir_all(&workspace).expect("建得了工作区");
    let opening = Opening {
        owner: owner.clone(),
        attended: false,
        oneshot: false,
        model: None,
        venue: None,
        memory: None,
        preset: None,
        group: None,
        profile: false,
        images: true,
        titles: false,
        origin: None,
    };
    core.sessions
        .create(
            core,
            CommandId::parse(command).expect("合写法"),
            Some(Some(SAMPLE_PERSONA)),
            workspace.to_string_lossy().into_owned(),
            Vec::new(),
            opening,
        )
        .await
        .expect("造得了")
        .id
}

/// 账号 `owner` 在会话 `session` 里说一句，等这一轮说完。
async fn say(core: &Arc<Core>, owner: &AccountId, session: &SessionId, command: &str, words: &str) {
    let found = core.sessions.get(core, session).await.expect("在跑");
    let ended = |dir: &std::path::Path| {
        read_events(dir)
            .unwrap_or_default()
            .iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
    };
    let dir = core.root.session_dir(owner, session);
    let before = ended(&dir);
    found
        .handle
        .command(
            CommandId::parse(command).expect("合写法"),
            By::Person(Person::new(owner.clone())),
            Command::Send {
                blocks: vec![Block::Text(Text {
                    text: words.to_string(),
                })],
                urgent: false,
                venue: None,
            },
        )
        .await
        .expect("会话在跑");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while ended(&dir) == before {
        assert!(tokio::time::Instant::now() < deadline, "等不到这一轮说完");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_member_s_session_remembers_dreams_and_recalls_in_his_own_room() {
    let script = Script::new([
        // 乙那一间整理：合出摘要（在整理会话里）。
        Play::Says(r#"{"summary": "乙养了一只狗。"}"#),
        // 乙新开的会话、甲新开的会话各说一轮。
        Play::Says("嗯"),
        Play::Says("嗯"),
    ])
    .once(Vec::<&str>::new());
    let core = core("accounts-member", &script);
    let mine = open(&core, &bob(), "o1").await;
    let found = core.sessions.get(&core, &mine).await.expect("在跑");
    assert_eq!(
        found.handle.memory_room(),
        Some(Room::persona(&bob(), SAMPLE_PERSONA)),
        "成员的会话有记忆，记在他自己的账号（系统区的人格也是）"
    );
    // 会话里记一条（`/remember` 那一路，照会话那一间）。
    super::super::remember_in(
        &core,
        &found.handle,
        &CommandId::parse("m1").expect("合写法"),
        By::Person(Person::new(bob())),
        "乙养了一只狗",
    )
    .await
    .expect("记得下");
    assert_eq!(texts(&core, &bob(), "l1").await, ["乙养了一只狗"]);
    // 载入的（核心重启以后、放下了再找回来）照样有记忆、照样是他那一间。
    core.sessions.forget(&mine).await;
    let loaded = core.sessions.get(&core, &mine).await.expect("载入得了");
    assert_eq!(
        loaded.handle.memory_room(),
        Some(Room::persona(&bob(), SAMPLE_PERSONA)),
        "载入的成员会话也有记忆"
    );
    assert!(
        texts(&core, &alice(), "l2").await.is_empty(),
        "机主看不到成员的"
    );
    assert!(
        core.root
            .account_dir(&bob())
            .join("modules/memory")
            .join(SAMPLE_PERSONA)
            .is_dir(),
        "记忆日志在乙的家目录里"
    );

    let dreamed = super::super::dream::call(&core, &bob(), &json!({"persona": SAMPLE_PERSONA}))
        .await
        .expect("整理了");
    assert_eq!(dreamed["summary"], json!(true), "{dreamed}");
    let dream =
        SessionId::parse(dreamed["session"].as_str().expect("跑在整理会话里")).expect("合写法");
    assert!(
        core.root.session_dir(&bob(), &dream).is_dir(),
        "整理会话归乙"
    );

    let next = open(&core, &bob(), "o2").await;
    say(&core, &bob(), &next, "s2", "早").await;
    let theirs = open(&core, &alice(), "o3").await;
    say(&core, &alice(), &theirs, "s3", "早").await;
    let requests = script.requests();
    let carried = |n: usize| format!("{:?}", requests[n].1).contains("乙养了一只狗。");
    assert_eq!(requests.len(), 3, "一批、两轮");
    assert!(carried(1), "乙新开的会话带上他的摘要");
    assert!(!carried(2), "甲的会话不带乙的");
}

#[tokio::test]
async fn memory_calls_find_the_room_of_whom_they_act_for() {
    let core = core("accounts-calls", &Script::new([]).once(Vec::<&str>::new()));
    for (who, text, id) in [
        (alice(), "甲养了一只猫", "r1"),
        (bob(), "乙养了一只狗", "r2"),
    ] {
        call(
            &core,
            &who,
            id,
            "memory.remember",
            json!({"persona": SAMPLE_PERSONA, "class": "user", "text": text}),
        )
        .await;
    }
    assert_eq!(texts(&core, &alice(), "l1").await, ["甲养了一只猫"]);
    assert_eq!(texts(&core, &bob(), "l2").await, ["乙养了一只狗"]);
    let searched = call(
        &core,
        &bob(),
        "q1",
        "memory.search",
        json!({"persona": SAMPLE_PERSONA, "query": "猫"}),
    )
    .await;
    assert_eq!(searched["memories"], json!([]), "乙搜不出甲的");

    // 无人格那一间、人卡也照替谁干活的账号。
    call(
        &core,
        &bob(),
        "r3",
        "memory.remember",
        json!({"persona": NO_PERSONA, "class": "user", "text": "乙在学吉他"}),
    )
    .await;
    let none = call(
        &core,
        &alice(),
        "l3",
        "memory.list",
        json!({"persona": NO_PERSONA}),
    )
    .await;
    assert_eq!(none["memories"], json!([]), "甲的无人格那一间没有乙的");
    call(
        &core,
        &bob(),
        "c1",
        "memory.card_update",
        json!({"persona": SAMPLE_PERSONA, "who": "qq:10086", "text": "小林是乙的群友。"}),
    )
    .await;
    let cards = |who: AccountId, id: &'static str| {
        let core = Arc::clone(&core);
        async move {
            call(
                &core,
                &who,
                id,
                "memory.cards",
                json!({"persona": SAMPLE_PERSONA}),
            )
            .await["cards"]
                .as_array()
                .expect("有 cards")
                .len()
        }
    };
    assert_eq!(cards(bob(), "k1").await, 1);
    assert_eq!(cards(alice(), "k2").await, 0, "甲看不到乙的人卡");

    // 只在乙的家目录里的人格：照乙看得到的几层找，甲找不到。
    let own = core.root.account_dir(&bob()).join("personas").join("mine");
    copy(&miyu_session::testkit::sample_persona_dir(), &own);
    call(
        &core,
        &bob(),
        "r4",
        "memory.remember",
        json!({"persona": "mine", "class": "user", "text": "乙的人格记得这个"}),
    )
    .await;
    let id = CommandId::parse("l4").expect("合写法");
    let refused = super::super::call(
        &core,
        &alice(),
        "memory.list",
        &id,
        &json!({"persona": "mine"}),
    )
    .await;
    assert!(refused.is_err(), "甲看不到乙家目录里的人格");

    // 记下的是替谁干活的那个人。
    let keeper = miyu_session::Keeper::new(
        &core.memory,
        Room::persona(&bob(), SAMPLE_PERSONA),
        miyu_session::Listener::Owner(By::Person(Person::new(bob()))),
    );
    let followed = keeper.follow(Some(0), 64).expect("跟得上");
    let by = &followed.filled.first().expect("有一条").by;
    assert_eq!(*by, By::Person(Person::new(bob())));
}

/// 整个目录照样复制一份。
fn copy(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("建得了");
    for entry in std::fs::read_dir(from).expect("读得了样本").flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("拷得了");
        }
    }
}
