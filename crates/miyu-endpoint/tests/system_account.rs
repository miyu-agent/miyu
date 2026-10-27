//! 系统账号（施工 O-4 下，`docs/construction/O-4-系统账号（下）.md`）：声明了系统账号的包，核心起来时建它的家目录和工作区；
//! 核心拉起的这个包的扩展以它的身份连进来：握手回应的 `account` 是它，群的场所会话归它（目录、工作目录、回应的 `account`），
//! 说完一轮用量记在它名下、记忆归管理员；主人的私聊照旧归主人，这个连接照样能对它说话；没声明系统账号的扩展也是自己的账号、
//! 替机主干活（施工 Y-5 中上），照旧回 `no_system_account`。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_kernel::id::AccountId;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;

use crate::support::extensions::*;
use crate::support::served::{replies, served_core};
use crate::support::venues::snapshot;
use crate::support::*;

fn bot() -> AccountId {
    AccountId::parse("bot").expect("合写法")
}

#[tokio::test]
async fn the_bridge_runs_as_its_system_account_and_owns_the_group() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let checked = home.work.join("group-checked");
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
            // 群会话归它：删也是它删（施工 Y-2 上起人的头碰不到场所会话，项目主人 10-23 定删放在 im 后台）。
            &format!("pause:{}", checked.display()),
            r#"ask:session.delete:{"session":"{session}"}"#,
            r#"ask:venue.session:{"venue":"qq:private:10001","kind":"private","peer":"qq:10001"}"#,
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

    assert_eq!(
        got[0]["result"]["account"], "bot",
        "握手：这个连接是系统账号"
    );
    let made = &got[1]["result"];
    assert_eq!(made["created"], true, "{got:?}");
    assert_eq!(made["account"], "bot", "群归系统账号");
    let session = made["session"].as_str().expect("有编号").to_string();
    assert_eq!(
        got[2]["result"],
        json!({"session": session, "created": false, "account": "bot"}),
        "照「场所加属主」找回同一个"
    );
    assert!(got[3].get("error").is_none(), "{got:?}");
    assert!(got[4].get("error").is_none(), "{got:?}");
    let groups = dir(&home, &bot(), &session);
    let created = read_events(&groups).expect("目录在 bot 的家目录下");
    let Body::SessionCreated(created) = &created[0].body else {
        panic!("第一条是 session.created");
    };
    assert_eq!(created.owner, bot());
    let workspace = home.root.workspace(&bot());
    assert!(workspace.is_dir(), "起来时建了它的工作区");
    let id = miyu_kernel::id::SessionId::parse(&session).expect("会话编号");
    let mine = home.root.session_workspace(&bot(), &id);
    assert_eq!(
        created.cwd.as_deref().map(Path::new),
        Some(mine.as_path()),
        "没写 cwd 的是这个会话自己的那一个（施工 5-14）"
    );
    assert!(mine.is_dir(), "建好了");
    // 群会话（施工 O-13 中）：快照钉下时区、system 接上格式说明。
    let note =
        std::fs::read_to_string(default_resources().join("core/venues/group.txt")).expect("读得到");
    let made = snapshot(&home, &bot(), &groups);
    assert!(made.group.is_some(), "群会话钉下时区");
    assert!(made.system.contains(note.trim_end()));
    // 群归系统账号以后，对应表认出的主人在群里说的是外部身份带账号，不是本人（`as` 照会话的属主比）。
    let owner_said = read_events(&groups)
        .expect("读得了")
        .into_iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.by)
        .find(|by| matches!(by, By::External(external) if external.id.as_str() == "qq:10001"));
    assert!(
        matches!(&owner_said, Some(By::External(external)) if external.account == Some(alice())),
        "{owner_said:?}"
    );

    until_turns_in(&groups, 1).await;
    // 会话落盘时就写：索引在它自己的家目录下、行里属主是它；用量记在它名下（查之前，不靠 `usage.query` 补）。
    let index = home.root.index(&bot()).join("sessions.db");
    until_counted(
        &index,
        "SELECT count(*) FROM sessions WHERE id = ?1 AND owner = 'bot'",
        &session,
    )
    .await;
    let ledger = home.root.state().join("usage.db");
    until_counted(
        &ledger,
        "SELECT count(*) FROM spent WHERE session = ?1 AND owner = 'bot'",
        &session,
    )
    .await;
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let usage = client
        .call("u1", "usage.query", json!({"group": ["account"]}))
        .await;
    let accounts: Vec<&str> = usage["result"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("{usage}"))
        .iter()
        .filter_map(|row| row["account"].as_str())
        .collect();
    assert!(
        accounts.contains(&"bot"),
        "用量记在系统账号名下、管理员看得到：{usage}"
    );

    // 记忆归管理员，系统账号没有自己的记忆。群里的回合进管理员那一间的回合库，记下它在哪个场所（施工 R-13 下，`memory.md`
    // 第一条第 1 款）；看删群会话时是不是照记忆归谁去埋墓碑。
    let turns = |account: &AccountId| {
        home.root
            .index(account)
            .join("recall")
            .join("turns-engineer.db")
    };
    // 回合库在后台建（施工 V-9）：等它出现，最多六十秒（机器忙时慢，只在出错时等满）。
    let built = tokio::time::Instant::now() + Duration::from_secs(60);
    while !turns(&alice()).exists() && tokio::time::Instant::now() < built {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!turns(&bot()).exists(), "系统账号没有自己的回合库");
    assert!(turns(&alice()).exists(), "群里的回合进管理员的回合库");
    // 删掉群会话（桥删）：管理员的回合库里给它埋墓碑，记忆的出处在它里面的都算死了。等它空下来再放桥删：跑着的删不掉。
    until_idle(&groups).await;
    std::fs::write(&checked, "").expect("写得进");
    let got = replies(&path, 8).await;
    assert_eq!(got[5]["result"], json!({}), "桥删得掉它自己的群：{got:?}");
    let db = rusqlite::Connection::open_with_flags(
        turns(&alice()),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
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
    assert_eq!(left, 0, "它自己的索引里那一行拿掉了");

    let private = &got[6]["result"];
    assert_eq!(
        private["account"], "alice",
        "对应表里的主人的私聊照旧归主人：{got:?}"
    );
    let mine = private["session"].as_str().expect("有编号").to_string();
    assert!(dir(&home, &alice(), &mine).is_dir());
    let made = snapshot(&home, &alice(), &dir(&home, &alice(), &mine));
    assert_eq!(made.group, None, "私聊不是群会话");
    assert!(!made.system.contains(note.trim_end()));
    assert!(
        got[7].get("error").is_none(),
        "系统账号的连接照样能对主人的会话说话：{got:?}"
    );

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

/// 起来以后才装上的包（施工 F-5 下）：当场建它的家目录、工作区，开它的会话列表索引，扩展以它的身份连进来。
#[tokio::test]
async fn a_package_installed_later_gets_its_system_account_at_once() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let script = Script::new([Play::Says("嗯。")]);
    let core = served_core(&home, &script);
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            r#"ask:venue.session:{"venue":"qq:group:1","kind":"group","persona":"engineer"}"#,
            r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"qq:20001"}}"#,
            "wait",
        ]),
        "always",
        true,
    );
    let source = home.work.join("bot");
    std::fs::rename(home.root.path().join("system/packages/bot"), &source).expect("挪得动");
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let reply = client
        .call("i1", "package.install", json!({"path": source}))
        .await;
    assert_eq!(reply["result"]["package"], "bot", "{reply}");

    let got = replies(&path, 3).await;
    assert_eq!(got[0]["result"]["account"], "bot", "{got:?}");
    assert_eq!(got[1]["result"]["account"], "bot", "{got:?}");
    assert!(got[2].get("error").is_none(), "{got:?}");
    assert!(home.root.workspace(&bot()).is_dir(), "装上就建了它的工作区");
    let session = got[1]["result"]["session"].as_str().expect("有编号");
    until_turns_in(&dir(&home, &bot(), session), 1).await;
    until_counted(
        &home.root.index(&bot()).join("sessions.db"),
        "SELECT count(*) FROM sessions WHERE id = ?1 AND owner = 'bot'",
        session,
    )
    .await;
    core.stop_extensions().await;
}

/// 起来时就卸掉了的出厂的包（施工 F-5 下）：装回来当场建它的家目录、工作区。
#[tokio::test]
async fn a_shipped_package_brought_back_gets_its_system_account_at_once() {
    // 出厂的扩展用测试自己放的夹具（施工 S-6 下：接入QQ 搬出了核心仓库）。
    let home = Home::new();
    home.write("system/packages/courier.removed", "");
    let resources = shipped::resources(&home);
    let core = Arc::new(shipped::core(&home, &resources, &Script::new([])));
    let courier = AccountId::parse(shipped::ID).expect("合写法");
    assert!(!home.root.workspace(&courier).exists(), "卸掉的不建");
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let reply = client
        .call("i1", "package.install", json!({"package": shipped::ID}))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(
        home.root.workspace(&courier).is_dir(),
        "装回来就建了它的工作区"
    );
    core.stop_extensions().await;
}

/// 没声明系统账号的扩展也是自己的账号、替机主干活（施工 Y-5 中上，U16）：开的会话归机主，照旧拿不到群会话。
#[tokio::test]
async fn an_extension_without_a_system_account_is_its_own_account_working_for_the_owner() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "plain");
    install_serving(
        &home,
        "plain",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            r#"ask:venue.session:{"venue":"qq:group:1","kind":"group"}"#,
            r#"ask:session.create:{}"#,
            "wait",
        ]),
        "always",
        false,
    );
    // 声明了系统账号、关着没拉起的：起来时照样建它的家目录和工作区。
    install_serving(&home, "quiet", &program.name(), &[], "manual", true);
    let core = served_core(&home, &Script::new([]));
    let quiet = AccountId::parse("quiet").expect("合写法");
    assert!(home.root.workspace(&quiet).is_dir(), "起来时建了它的工作区");
    let plain = AccountId::parse("plain").expect("合写法");
    let got = replies(&path, 3).await;
    assert_eq!(
        got[0]["result"]["account"], "plain",
        "握手：这个连接是包自己的账号"
    );
    assert_eq!(reason(&got[1]), Some("no_system_account"), "{got:?}");
    assert!(home.root.workspace(&plain).is_dir(), "建了家目录");
    let session = got[2]["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{got:?}"))
        .to_string();
    let made = read_events(&dir(&home, &alice(), &session)).expect("在机主名下");
    let Body::SessionCreated(created) = &made[0].body else {
        panic!("第一条是 session.created");
    };
    assert_eq!(created.owner, alice(), "替机主干活：会话属主是机主");
    core.stop_extensions().await;
}

/// 用量汇总丢了（派生的，删掉重建）：`usage.query` 先补，系统账号名下的会话也照日志补回来。
#[tokio::test]
async fn usage_of_a_system_account_is_caught_up_after_the_ledger_is_lost() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let script = Script::new([Play::Says("好。")]);
    let bridge = |start: &str| {
        install_serving(
            &home,
            "bot",
            &program.name(),
            &steps(&[
                &step,
                "hello",
                r#"ask:venue.session:{"venue":"qq:group:1","kind":"group"}"#,
                r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"qq:20001"}}"#,
                "wait",
            ]),
            start,
            true,
        );
    };
    bridge("always");
    let first = served_core(&home, &script);
    let got = replies(&path, 3).await;
    let session = got[1]["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{got:?}"))
        .to_string();
    until_turns_in(&dir(&home, &bot(), &session), 1).await;
    first.stop_extensions().await;
    first.stop_sessions().await;
    drop(first);
    // 删掉汇总再起来：关着不拉起，只剩日志。Windows 上开着的删不掉，照旧的读也一样过。
    for name in ["usage.db", "usage.db-wal", "usage.db-shm"] {
        drop(std::fs::remove_file(home.root.state().join(name)));
    }
    bridge("manual");
    let second = served_core(&home, &Script::new([]));
    let mut client = Client::connect(second.clone());
    client.hello().await;
    let usage = client
        .call("u1", "usage.query", json!({"group": ["account"]}))
        .await;
    let accounts: Vec<&str> = usage["result"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("{usage}"))
        .iter()
        .filter_map(|row| row["account"].as_str())
        .collect();
    assert!(accounts.contains(&"bot"), "照它的日志补回来：{usage}");
}

/// `events.append`（施工 O-13 上）：核心拉起的扩展只能写自己的包那一段的 `ext.*`。
#[tokio::test]
async fn an_extension_writes_only_its_own_ext_events() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            r#"ask:venue.session:{"venue":"qq:group:1","kind":"group"}"#,
            r#"ask:events.append:{"session":"{session}","kind":"ext.bot.chat.decided","body":{"to":[1]}}"#,
            r#"ask:events.append:{"session":"{session}","kind":"ext.other.chat.decided","body":{}}"#,
            "wait",
        ]),
        "always",
        true,
    );
    let core = served_core(&home, &Script::new([]));
    let got = replies(&path, 4).await;
    assert!(got[2]["result"]["seq"].is_u64(), "自己的包：{got:?}");
    assert_eq!(reason(&got[3]), Some("bad_params"), "别的包的：{got:?}");
    let session = got[1]["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let decided = read_events(&dir(&home, &bot(), &session))
        .expect("读得了")
        .into_iter()
        .find(|event| event.body.kind() == "ext.bot.chat.decided")
        .expect("记下了");
    assert_eq!(
        serde_json::to_value(&decided.by).expect("写得出"),
        json!({"kind": "module", "id": "bot"}),
        "记成这个包写的"
    );
    core.stop_extensions().await;
}
