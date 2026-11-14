//! 主人只读地看场所会话（施工 VW-1，`docs/construction/VW-1-主人只读看场所会话.md`）：人的头对关着的群会话订阅（连补发）、
//! 读页、带会话取图都成；发话、重做、换配置照旧 `session_not_found`，会话日志一条不多。平台桥对别的平台的会话照旧碰不到，
//! 读的也不行。另照协议的方法表逐个守：表外的方法人的头对关着的场所会话一律当没有这个会话，以后加了方法也逃不掉。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::id::ContentHash;
use miyu_session::testkit::{Play, Script};

use crate::attach::png;
use crate::blob_session::{got, on_qq, replies, served_core_bound};
use crate::support::extensions::*;
use crate::support::venues::BINDINGS;
use crate::support::*;

/// 人的头对关着的场所会话能调的，同 `sessions/peek.rs` 的 `READS`：两边要一起改。
const READS: [&str; 11] = [
    "subscribe",
    "unsubscribe",
    "view.page",
    "view.detail",
    "session.prompts",
    "session.tools",
    "session.context",
    "session.usage_parts",
    "job.output",
    "blob.get",
    "session.shown",
];

/// 起好的一个关着的群会话：核心、会话编号、桥发进去的图、桥记下的回应在哪。
struct Group {
    core: Arc<Core>,
    session: String,
    picture: Vec<u8>,
    blob: String,
    record: PathBuf,
}

/// 接 QQ 的桥开一个群会话，以群友的身份发一张图进去，答完以后再接着走 `then` 那几步。对应表另认 `tg:10001` 是主人。
async fn group(home: &Home, then: &[&str]) -> Group {
    let program = Program::new();
    let (record, step) = record(home, "bot");
    let picture = png(2, 2);
    let blob = ContentHash::of(&picture).as_str().to_string();
    let mut all = vec![
        step,
        "hello".to_string(),
        r#"ask:venue.session:{"venue":"qq:group:1","kind":"group","persona":"engineer"}"#
            .to_string(),
        format!(
            r#"ask:blob.put:{{"data":"{}","name":"a.png"}}"#,
            STANDARD.encode(&picture)
        ),
        format!(
            r#"ask:session.send:{{"session":"{{session}}","text":"看图","as":{{"external":"qq:20001"}},"attachments":[{{"blob":"{blob}","name":"a.png","media_type":"image/png"}}]}}"#
        ),
    ];
    all.extend(then.iter().map(ToString::to_string));
    all.push("wait".to_string());
    install_serving(home, "bot", &program.name(), &all, "always", true);
    on_qq(home);
    let bindings = format!("{BINDINGS}\"tg:10001\" = \"admin\"\n");
    let core = served_core_bound(home, &Script::new([Play::Says("嗯。")]), &bindings);
    let made = replies(&record, 4).await;
    assert!(made[3].get("error").is_none(), "桥发进它的群：{made:?}");
    let session = made[1]["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{made:?}"))
        .to_string();
    until_turns_in(&dir(home, &session), 1).await;
    Group {
        core,
        session,
        picture,
        blob,
        record,
    }
}

/// 协议的方法：`docs/reference/methods-v1.txt` 冻结的那一份（门禁照代码查它全不全），加上连接上办的订阅、退订。
fn methods() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference/methods-v1.txt");
    let text = std::fs::read_to_string(&path).expect("读得到方法表");
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .chain(["subscribe", "unsubscribe"].map(str::to_string))
        .collect()
}

#[tokio::test]
async fn the_owner_reads_a_confined_group_but_cannot_drive_it() {
    let home = Home::new();
    let elsewhere = home.work.join("elsewhere");
    let blob = ContentHash::of(&png(2, 2)).as_str().to_string();
    let ask = format!(r#"ask:blob.get:{{"blob":"{blob}","session":"{{session}}"}}"#);
    let then = [
        format!("session:{}", elsewhere.display()),
        r#"ask:view.page:{"session":"{session}"}"#.to_string(),
        r#"ask:subscribe:{"session":"{session}","stream":"view"}"#.to_string(),
        ask,
    ];
    let group = group(&home, &then.each_ref().map(String::as_str)).await;
    let session = group.session.as_str();
    let mut client = Client::connect(group.core.clone());
    client.hello().await;
    let before = home.log(session).len();

    // 订阅连补发：从头补来的有群友那一句。
    let (pushed, reply) = client.subscribe_after("e1", session, json!(0)).await;
    assert!(
        reply.get("error").is_none(),
        "人的头订得到关着的群：{reply}"
    );
    assert!(
        pushed
            .iter()
            .any(|push| push["params"]["event"]["kind"] == "message.user"),
        "补来了群里的话：{pushed:?}"
    );
    let page = client
        .call("p1", "view.page", json!({"session": session, "view": true}))
        .await;
    let entries = page["result"]["entries"].as_array();
    assert!(entries.is_some_and(|e| !e.is_empty()), "读得到页：{page}");
    let picture = client
        .call(
            "b1",
            "blob.get",
            json!({"blob": group.blob, "session": session}),
        )
        .await;
    assert_eq!(got(&picture), Ok(group.picture.clone()), "图取得到");

    // 写的照旧当没有这个会话。
    let writes = [
        ("session.send", json!({"session": session, "text": "在吗"})),
        ("session.redo", json!({"session": session})),
        (
            "session.configure",
            json!({"session": session, "persona": null}),
        ),
    ];
    for (n, (method, params)) in writes.into_iter().enumerate() {
        let reply = client.call(&format!("w{n}"), method, params).await;
        assert_eq!(
            reason(&reply),
            Some("session_not_found"),
            "{method}：{reply}"
        );
    }
    assert_eq!(home.log(session).len(), before, "群会话的日志一条不多");

    // 主人在别的平台上的私聊：接 QQ 的桥读的也碰不到。
    let tg = client
        .call(
            "t1",
            "venue.session",
            json!({"venue": "tg:private:10001", "kind": "private", "peer": "tg:10001"}),
        )
        .await;
    let tg = tg["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{tg}"));
    std::fs::write(&elsewhere, tg).expect("写得进");
    let read = replies(&group.record, 7).await;
    assert_eq!(reason(&read[4]), Some("session_not_found"), "{read:?}");
    assert_eq!(reason(&read[5]), Some("session_not_found"), "{read:?}");
    assert_eq!(got(&read[6]), Err("unknown_blob".to_string()), "{read:?}");
    group.core.stop_extensions().await;
}

/// 协议里的每一个方法带上关着的群会话的编号：只读的那几个不回 `session_not_found`，别的一律回，会话的日志一条不多。
#[tokio::test]
async fn every_method_outside_the_reads_is_refused_on_a_confined_group() {
    let home = Home::new();
    let group = group(&home, &[]).await;
    let session = group.session.as_str();
    let methods = methods();
    for read in READS {
        assert!(methods.contains(read), "只读的 {read} 是协议里的方法");
    }
    let mut client = Client::connect(group.core.clone());
    client.hello().await;
    let before = home.log(session).len();
    for (n, method) in methods.iter().enumerate() {
        let reply: Value = client
            .call(&format!("m{n}"), method, json!({"session": session}))
            .await;
        if READS.contains(&method.as_str()) {
            assert_ne!(
                reason(&reply),
                Some("session_not_found"),
                "{method}：{reply}"
            );
        } else {
            assert_eq!(
                reason(&reply),
                Some("session_not_found"),
                "{method}：{reply}"
            );
        }
    }
    assert_eq!(home.log(session).len(), before, "群会话的日志一条不多");
    group.core.stop_extensions().await;
}
