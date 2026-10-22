//! 工作目录（施工 4-3 下，`11-权限与沙盒.md` 第四节；施工 5-14 起工作目录和工作区分开）：头报来的是 `~`、系统的家目录、
//! 根目录、包含数据根的，照样是工作目录、回应带 `wide`（能写的工作区由会话退回它的默认工作区）；落在数据根里的、读不出家目录
//! 的 `~`，退回这个会话的默认工作区 `home/<账号>/workspace/<会话>/`；账号的工作区本身（网页不选目录时交的）造会话时也换成
//! 它；项目目录照旧。造会话、说话的回应说会话实际在哪个目录里（施工 4-5 下）。施工 9-7 上起换工作区的是 `session.set_workspace`；说话带的 `cwd` 9-7 再补起
//! 先换过去（`workspace_send.rs`）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use crate::support::{Client, Home, alice};
use miyu_kernel::template::escape;
use miyu_session::testkit::{Play, Script};

/// 一个用完就删的临时目录：假的家、项目目录放在数据根外面。
struct Outside(PathBuf);

impl Outside {
    fn new() -> Outside {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-endpoint-out-{}-{n}", std::process::id()));
        for sub in ["home", "proj"] {
            std::fs::create_dir_all(dir.join(sub)).expect("建得了目录");
        }
        Outside(dir)
    }
}

impl Drop for Outside {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 请求里全部的字，接成一段。
fn words(value: &Value, out: &mut String) {
    match value {
        Value::String(text) => out.push_str(text),
        Value::Array(items) => items.iter().for_each(|item| words(item, out)),
        Value::Object(fields) => fields.values().for_each(|field| words(field, out)),
        _ => {}
    }
}

/// 头报来的工作目录是 `cwd`：她看到的请求里的字。每次起一个新的核心，数据根是同一个：命令编号要全局不撞
/// （`04-核心协议.md` 第六节第 1 条），核心重启以后照样认得出重发的造会话，所以每次换一对编号。
async fn seen(home: &Home, outside: &Outside, cwd: &str) -> (String, String) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core_at_home(&script, outside.0.join("home")));
    client.hello().await;
    let session = client.create(&format!("create-{n}"), cwd).await;
    client
        .call(
            &format!("send-{n}"),
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    home.until_turns(&session, 1).await;
    let request = script.requests()[0].1.clone();
    let value: Value = serde_json::from_slice(&request.canonical_bytes()).expect("请求是 JSON");
    let mut text = String::new();
    words(&value, &mut text);
    (session, text)
}

/// 会话 `session` 的默认工作区。
fn own_of(home: &Home, session: &str) -> PathBuf {
    let id = miyu_kernel::id::SessionId::parse(session).expect("会话编号");
    home.root.session_workspace(&alice(), &id)
}

/// 环境事实里那一格。
fn cwd_fact(path: &Path) -> String {
    format!("cwd=\"{}\"", written(path))
}

/// 环境事实里写的工作目录：照事实模板转义过。
fn written(path: &Path) -> String {
    escape(&path.to_string_lossy())
}

#[tokio::test]
async fn a_too_wide_directory_stays_where_the_person_is() {
    let home = Home::new();
    let outside = Outside::new();
    let root = if cfg!(windows) { "C:\\" } else { "/" };
    for cwd in [
        "~".to_string(),
        outside.0.join("home").to_string_lossy().into_owned(),
        root.to_string(),
        home.root
            .path()
            .parent()
            .expect("数据根有上一级")
            .to_string_lossy()
            .into_owned(),
    ] {
        let (_, text) = seen(&home, &outside, &cwd).await;
        assert!(text.contains(&cwd_fact(Path::new(&cwd))), "{cwd}：{text}");
    }
    // 数据根本身、落在数据根里的：退回这个会话的默认工作区，建好。
    for inside_data in [
        home.root.path().to_path_buf(),
        home.root.path().join("state"),
    ] {
        let (session, text) = seen(&home, &outside, &inside_data.to_string_lossy()).await;
        let own = own_of(&home, &session);
        assert!(text.contains(&cwd_fact(&own)), "{text}");
        assert!(own.is_dir(), "退回的工作区建好了");
    }
    // 核心读不出家目录：`~` 说不清在哪，也退回。
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c9", "~").await;
    client
        .call(
            "c10",
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    home.until_turns(&session, 1).await;
    let value: Value =
        serde_json::from_slice(&script.requests()[0].1.canonical_bytes()).expect("请求是 JSON");
    let mut text = String::new();
    words(&value, &mut text);
    assert!(text.contains(&cwd_fact(&own_of(&home, &session))), "{text}");
}

#[tokio::test]
async fn a_project_directory_stays_and_the_account_workspace_becomes_the_session_s() {
    let home = Home::new();
    let outside = Outside::new();
    let project = outside.0.join("proj");
    let (_, text) = seen(&home, &outside, &project.to_string_lossy()).await;
    assert!(text.contains(&cwd_fact(&project)), "{text}");
    // 账号的工作区本身（网页不选目录时交的）：换成这个会话自己的那一个，建好（施工 5-14）。
    let own = home.root.workspace(&alice());
    std::fs::create_dir_all(&own).expect("建得了");
    let (session, text) = seen(&home, &outside, &own.to_string_lossy()).await;
    let mine = own_of(&home, &session);
    assert!(text.contains(&cwd_fact(&mine)), "{text}");
    assert!(mine.is_dir(), "{}", mine.display());
    // 账号的工作区里面别的目录（以前的会话、别的会话的那一个）照原样。
    let (_, text) = seen(&home, &outside, &mine.to_string_lossy()).await;
    assert!(text.contains(&cwd_fact(&mine)), "{text}");
}

/// `~` 的回应：Windows 上太宽、带 `wide`，别处不再太宽、不带（施工 5-14 改）。
fn home_wide(reply: &serde_json::Value) {
    let wide = reply["result"].get("wide");
    if cfg!(windows) {
        assert_eq!(wide, Some(&json!(true)), "{reply}");
    } else {
        assert!(wide.is_none(), "{reply}");
    }
}

#[tokio::test]
async fn the_replies_say_where_the_session_works() {
    let home = Home::new();
    let outside = Outside::new();
    let project = outside.0.join("proj").to_string_lossy().into_owned();
    let script = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let mut client = Client::connect(home.core_at_home(&script, outside.0.join("home")));
    client.hello().await;
    let reply = client
        .call("c1", "session.create", json!({"cwd": project}))
        .await;
    assert_eq!(
        reply["result"]["cwd"],
        json!(project),
        "项目目录照原样：{reply}"
    );
    let session = reply["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    // 说话带着的目录先换过去（施工 9-7 再补，终端无缝集成；9-7 上那时照收不理）：照人选的用，太宽的回应带 `wide`（`~` 只有
    // Windows 上还太宽，施工 5-14 改）。
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "hi", "cwd": "~"}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!("~"), "{reply}");
    home_wide(&reply);
    home.until_turns(&session, 1).await;
    // 人明着换到太宽的目录：照人选的用，回应带 `wide`（施工 9-7 补，原来退回账号的工作区）。
    let reply = client
        .call(
            "w1",
            "session.set_workspace",
            json!({"session": session, "cwd": "~"}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!("~"), "{reply}");
    home_wide(&reply);
    // 之后说话的回应，是会话现在的。
    let reply = client
        .call(
            "c3",
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!("~"), "{reply}");
    home.until_turns(&session, 2).await;
    // 造会话时就太宽的、头自己带上的：照用，带 `wide`（施工 5-14，原来退回账号的工作区）；同一个命令编号重发的，说的一样。
    for _ in 0..2 {
        let reply = client
            .call("c4", "session.create", json!({"cwd": "~"}))
            .await;
        assert_eq!(reply["result"]["cwd"], json!("~"), "{reply}");
        home_wide(&reply);
    }
    // 根目录哪儿都太宽：照用，带 `wide`。
    let root = if cfg!(windows) { "C:\\" } else { "/" };
    let reply = client
        .call("c8", "session.create", json!({"cwd": root}))
        .await;
    assert_eq!(reply["result"]["cwd"], json!(root), "{reply}");
    assert_eq!(reply["result"]["wide"], json!(true), "{reply}");
    // 带 `chosen` 的照收，说的一样。
    let reply = client
        .call("c5", "session.create", json!({"cwd": "~", "chosen": true}))
        .await;
    assert_eq!(reply["result"]["cwd"], json!("~"), "{reply}");
    home_wide(&reply);
    // 明着选了数据根里面、又不是账号自己的工作区的：照不了，退回这个会话的默认工作区。
    let state = home.root.state().to_string_lossy().into_owned();
    let reply = client
        .call(
            "c7",
            "session.create",
            json!({"cwd": state, "chosen": true}),
        )
        .await;
    let session = reply["result"]["session"].as_str().expect("造出来了");
    let mine = own_of(&home, session).to_string_lossy().into_owned();
    assert_eq!(reply["result"]["cwd"], json!(mine), "{reply}");
    assert!(reply["result"].get("wide").is_none(), "{reply}");
    // 不太宽的明着选，和不选一样，不带 `wide`。
    let reply = client
        .call(
            "c6",
            "session.create",
            json!({"cwd": project, "chosen": true}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!(project), "{reply}");
    assert!(reply["result"].get("wide").is_none(), "{reply}");
}

#[tokio::test]
async fn a_venue_session_without_a_directory_works_in_its_own_folder() {
    let home = Home::new();
    let outside = Outside::new();
    home.write("system/config.toml", crate::support::venues::BINDINGS);
    let items = [
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    // 核心读得出家目录：场所会话没写 `cwd` 的 `~` 太宽，不照用，用这个会话自己的那一个（施工 5-14）。
    let core = std::sync::Arc::new(
        home.core_full(
            &Script::new([]),
            miyu_tool::Catalog::default(),
            Some(outside.0.join("home")),
            crate::support::TOKEN,
        )
        .with_config(config),
    );
    let mut client = Client::connect(core);
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{made}"))
        .to_string();
    let created = home.log(&session);
    let miyu_kernel::event::Body::SessionCreated(created) = &created[0].body else {
        panic!("第一条是 session.created");
    };
    let mine = own_of(&home, &session);
    assert_eq!(created.cwd.as_deref().map(Path::new), Some(mine.as_path()));
    assert!(mine.is_dir());
}

#[tokio::test]
async fn the_account_workspace_means_the_session_s_own_and_replies_say_which_it_is() {
    let home = Home::new();
    let outside = Outside::new();
    let project = outside.0.join("proj").to_string_lossy().into_owned();
    let core = home.core_at_home(&Script::new([]), outside.0.join("home"));
    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", &project).await;
    let own = own_of(&home, &session).to_string_lossy().into_owned();
    // 订阅（另一个连接：订阅着的连接先收推送）、会话列表都说这个会话的默认工作区（施工 5-14 中）。
    let mut watcher = Client::connect(core);
    watcher.hello().await;
    let reply = watcher.subscribe("s1", &session).await;
    assert_eq!(reply["result"]["default_workspace"], json!(own), "{reply}");
    let listed = client.call("l1", "session.list", json!({})).await;
    let row = &listed["result"]["sessions"][0];
    assert_eq!(
        (&row["cwd"], &row["default_workspace"]),
        (&json!(project), &json!(own)),
        "{listed}"
    );
    // 网页的「默认工作区」交账号的工作区本身：换成会话自己的那一个，建好。
    let account = home.root.workspace(&alice()).to_string_lossy().into_owned();
    let reply = client
        .call(
            "w1",
            "session.set_workspace",
            json!({"session": session, "cwd": account}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!(own), "{reply}");
    assert!(reply["result"].get("wide").is_none(), "{reply}");
    assert!(Path::new(&own).is_dir());
    let listed = client.call("l2", "session.list", json!({})).await;
    let row = &listed["result"]["sessions"][0];
    assert_eq!(row["cwd"], row["default_workspace"], "{listed}");
}

/// 只用 Landlock 的机器上（这个测试的进程没设挂载），工作目录包住数据根的：回应多 `unguarded: "no_bwrap"`，造会话、换工作区都说
/// （施工 5-14 改）；不包住数据根的不说。
#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_place_holding_the_data_root_is_said_to_be_unguarded() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let holder = home
        .root
        .path()
        .parent()
        .expect("数据根有上级")
        .to_string_lossy()
        .into_owned();
    let reply = client
        .call("c1", "session.create", json!({"cwd": holder}))
        .await;
    assert_eq!(reply["result"]["cwd"], json!(holder), "{reply}");
    assert_eq!(reply["result"]["unguarded"], json!("no_bwrap"), "{reply}");
    let session = reply["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    let project = home.work.join("proj");
    std::fs::create_dir_all(&project).expect("建得了");
    let project = project.to_string_lossy().into_owned();
    let reply = client
        .call(
            "w1",
            "session.set_workspace",
            json!({"session": session, "cwd": project}),
        )
        .await;
    assert!(reply["result"].get("unguarded").is_none(), "{reply}");
    let reply = client
        .call(
            "w2",
            "session.set_workspace",
            json!({"session": session, "cwd": holder}),
        )
        .await;
    assert_eq!(reply["result"]["unguarded"], json!("no_bwrap"), "{reply}");
}
