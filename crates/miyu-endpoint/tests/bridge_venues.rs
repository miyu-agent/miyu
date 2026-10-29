//! 平台桥和场所会话（施工 O-4 下起；施工 SU-4 中）：核心拉起的、清单写了平台的包是桥；它开的群会话
//! 关起来（`session.created` 的 `confined`），找回照「场所加关不关」；对应表里的主人在群里说话照旧是外部身份；
//! 用量、回合库都照记；主人的私聊不关，桥照样能对它说话；人的头碰不到关着的群会话、碰得到主人的私聊，桥碰不到本机的会话；
//! 桥只能写自己的包那一段的 `ext.*`。

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;

use crate::support::extensions::*;
use crate::support::served::{replies, served_core};
use crate::support::venues::snapshot;
use crate::support::*;

#[tokio::test]
async fn the_bridge_opens_groups_under_the_owner_confined() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let checked = home.work.join("group-checked");
    let local = home.work.join("local-session");
    let group = r#"{"venue":"qq:group:1","kind":"group","persona":"engineer"}"#;
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!("ask:venue.session:{group}"),
            &format!("ask:venue.session:{group}"),
            r#"ask:session.send:{"session":"{session}","text":"我的显卡是 N 卡","as":{"external":"qq:20001"}}"#,
            r#"ask:session.send:{"session":"{session}","text":"我也在","as":{"external":"qq:10001"}}"#,
            // 群会话只有桥碰得到：删也是它删（项目主人 10-23 定删放在 im 后台）。
            &format!("pause:{}", checked.display()),
            r#"ask:session.delete:{"session":"{session}"}"#,
            r#"ask:venue.session:{"venue":"qq:private:10001","kind":"private","peer":"qq:10001"}"#,
            r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"qq:10001"}}"#,
            &format!("session:{}", local.display()),
            r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"qq:10001"}}"#,
            "wait",
        ]),
        "always",
        true,
    );
    let script = Script::new([
        Play::Says("好，记住你用 N 卡。"),
        Play::Says("嗯。"),
        Play::Says("在。"),
        Play::Says("在。"),
        Play::Says("嗯。"),
    ]);
    let core = served_core(&home, &script);
    let got = replies(&path, 5).await;

    assert!(got[0].get("result").is_some(), "握手成了：{got:?}");
    let made = &got[1]["result"];
    assert_eq!(made["created"], true, "{got:?}");
    assert!(made.get("account").is_none(), "回应不带属主：{got:?}");
    let session = made["session"].as_str().expect("有编号").to_string();
    assert_eq!(
        got[2]["result"],
        json!({"session": session, "created": false}),
        "照「场所加关不关」找回同一个"
    );
    assert!(got[3].get("error").is_none(), "{got:?}");
    assert!(got[4].get("error").is_none(), "{got:?}");
    let groups = dir(&home, &session);
    let created = read_events(&groups).expect("目录在主人的家目录下");
    let Body::SessionCreated(created) = &created[0].body else {
        panic!("第一条是 session.created");
    };
    assert!(created.confined, "群关起来");
    let id = miyu_kernel::id::SessionId::parse(&session).expect("会话编号");
    let mine = home.root.session_workspace(&id);
    assert_eq!(
        created.cwd.as_deref().map(Path::new),
        Some(mine.as_path()),
        "没写 cwd 的是这个会话自己的那一个（施工 5-14）"
    );
    assert!(mine.is_dir(), "建好了");
    // 群会话（施工 O-13 中）：快照钉下时区、system 接上格式说明。
    let note =
        std::fs::read_to_string(default_resources().join("core/venues/group.txt")).expect("读得到");
    let made = snapshot(&home, &groups);
    assert!(made.group.is_some(), "群会话钉下时区");
    assert!(made.system.contains(note.trim_end()));
    // 对应表认出的主人在群里说的照旧是外部身份带 `owner`，不是本人（`as` 照关不关比，施工 SU-4 中）。
    let owner_said = read_events(&groups)
        .expect("读得了")
        .into_iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.by)
        .find(|by| matches!(by, By::External(external) if external.id.as_str() == "qq:10001"));
    assert!(
        matches!(&owner_said, Some(By::External(external)) if external.owner),
        "{owner_said:?}"
    );

    until_turns_in(&groups, 1).await;
    // 会话落盘时就写：索引在数据根下；用量也记下（查之前，不靠 `usage.query` 补）。
    let index = home.root.index().join("sessions.db");
    until_counted(
        &index,
        "SELECT count(*) FROM sessions WHERE id = ?1",
        &session,
    )
    .await;
    let ledger = home.root.state().join("usage.db");
    until_counted(
        &ledger,
        "SELECT count(*) FROM spent WHERE session = ?1",
        &session,
    )
    .await;
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let usage = client
        .call("u1", "usage.query", json!({"group": ["venue"]}))
        .await;
    let venues: Vec<&str> = usage["result"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("{usage}"))
        .iter()
        .filter_map(|row| row["venue"].as_str())
        .collect();
    assert!(venues.contains(&"qq:group:1"), "群的用量照场所记：{usage}");

    // 记忆归主人。群里的回合进主人那一间的回合库，记下它在哪个场所（施工 R-13 下，`memory.md`
    // 第一条第 1 款）；看删群会话时是不是照记忆归谁去埋墓碑。
    let turns = || home.root.index().join("recall").join("turns-engineer.db");
    // 回合库在后台建（施工 V-9）：等它出现，最多六十秒（机器忙时慢，只在出错时等满）。
    let built = tokio::time::Instant::now() + Duration::from_secs(60);
    while !turns().exists() && tokio::time::Instant::now() < built {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(turns().exists(), "群里的回合进主人的回合库");
    // 人的头碰不到关着的群会话（施工 Y-2 上；SU-4 中起照关不关）。
    let reply = client.subscribe("g1", &session).await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
    // 删掉群会话（桥删）：回合库里给它埋墓碑，记忆的出处在它里面的都算死了。等它空下来再放桥删：跑着的删不掉。
    until_idle(&groups).await;
    std::fs::write(&checked, "").expect("写得进");
    let got = replies(&path, 8).await;
    assert_eq!(got[5]["result"], json!({}), "桥删得掉它自己的群：{got:?}");
    let db =
        rusqlite::Connection::open_with_flags(turns(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("回合库在");
    let buried: i64 = db
        .query_row(
            "SELECT count(*) FROM buried WHERE key = ?1",
            [format!("{session}/")],
            |row| row.get(0),
        )
        .expect("查得了");
    assert_eq!(buried, 1, "照记忆归谁去拿掉");
    let left: i64 =
        rusqlite::Connection::open_with_flags(&index, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .and_then(|db| {
                db.query_row(
                    "SELECT count(*) FROM sessions WHERE id = ?1",
                    [&session],
                    |row| row.get(0),
                )
            })
            .expect("索引在");
    assert_eq!(left, 0, "索引里那一行拿掉了");

    let private = &got[6]["result"];
    let mine = private["session"].as_str().expect("有编号").to_string();
    let first = read_events(&dir(&home, &mine)).expect("在主人名下");
    let Body::SessionCreated(opened) = &first[0].body else {
        panic!("第一条是 session.created");
    };
    assert!(!opened.confined, "对应表里的主人的私聊不关：{got:?}");
    let made = snapshot(&home, &dir(&home, &mine));
    assert_eq!(made.group, None, "私聊不是群会话");
    assert!(!made.system.contains(note.trim_end()));
    assert!(
        got[7].get("error").is_none(),
        "桥照样能对主人的私聊说话：{got:?}"
    );
    let reply = client.subscribe("g2", &mine).await;
    assert!(
        reply.get("error").is_none(),
        "主人的私聊不关，头碰得到：{reply}"
    );
    // 桥碰不到本机的会话（照场所编号的平台前缀，施工 SU-4 中）。
    let here = client.create("c1", "~").await;
    std::fs::write(&local, &here).expect("写得进");
    let got = replies(&path, 9).await;
    assert_eq!(reason(&got[8]), Some("session_not_found"), "{got:?}");

    core.stop_extensions().await;
}

/// 等会话目录 `dir` 里的人话都排完、最后一轮结束了。
async fn until_idle(dir: &Path) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let events = read_events(dir).unwrap_or_default();
        let last_said = events
            .iter()
            .filter(|event| matches!(event.body, Body::MessageUser(_)))
            .map(|event| event.seq)
            .max();
        let last_ended = events
            .iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .map(|event| event.seq)
            .max();
        let started = events
            .iter()
            .filter(|event| matches!(event.body, Body::TurnStarted(_)))
            .count();
        let ended = events
            .iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count();
        if started == ended && last_ended > last_said {
            return;
        }
        assert!(tokio::time::Instant::now() < deadline, "一直没空下来");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// `events.append`（施工 O-13 上）：核心拉起的扩展只能写自己的包那一段的 `ext.*`。
#[tokio::test]
async fn an_extension_writes_only_its_own_ext_events() {
    writes_only_its_own("bot", None, "ext.other.chat.decided").await;
}

/// 包的前缀（施工 PN-1，设计 48 第二节）：写了 `[package] prefix` 的，`ext.<前缀>.*` 是它的，`ext.<包编号>.*` 不是。
#[tokio::test]
async fn an_extension_writes_ext_events_under_its_prefix() {
    writes_only_its_own("miyu-bot", Some("bot"), "ext.miyu-bot.chat.decided").await;
}

/// 包 `id`（前缀 `prefix`，没写的照编号）写一条 `ext.bot.chat.decided` 和一条 `other`：前一条记下、记成这个包写的，后一条
/// `bad_params`。
async fn writes_only_its_own(id: &str, prefix: Option<&str>, other: &str) {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, id);
    let other =
        format!(r#"ask:events.append:{{"session":"{{session}}","kind":"{other}","body":{{}}}}"#);
    install_serving(
        &home,
        id,
        &program.name(),
        &steps(&[
            &step,
            "hello",
            r#"ask:venue.session:{"venue":"qq:group:1","kind":"group"}"#,
            r#"ask:events.append:{"session":"{session}","kind":"ext.bot.chat.decided","body":{"to":[1]}}"#,
            &other,
            "wait",
        ]),
        "always",
        true,
    );
    if let Some(prefix) = prefix {
        let manifest = format!("packages/{id}/package.toml");
        let text = read(&home.root.path().join(&manifest));
        home.write(
            &manifest,
            &text.replacen(
                "[package]\n",
                &format!("[package]\nprefix = \"{prefix}\"\n"),
                1,
            ),
        );
    }
    let core = served_core(&home, &Script::new([]));
    let got = replies(&path, 4).await;
    assert!(got[2]["result"]["seq"].is_u64(), "自己的：{got:?}");
    assert_eq!(reason(&got[3]), Some("bad_params"), "不是自己的：{got:?}");
    let session = got[1]["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let decided = read_events(&dir(&home, &session))
        .expect("读得了")
        .into_iter()
        .find(|event| event.body.kind() == "ext.bot.chat.decided")
        .expect("记下了");
    assert_eq!(
        serde_json::to_value(&decided.by).expect("写得出"),
        json!({"kind": "module", "id": id}),
        "记成这个包写的"
    );
    core.stop_extensions().await;
}
