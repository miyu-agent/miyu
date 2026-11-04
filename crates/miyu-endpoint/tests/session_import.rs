//! 导入原始包（施工 SE-1 下，`docs/blueprint/protocol.md`「`session.import`」）：真核心走一遍。一个数据根导出、另一个导入，
//! 日志一个字节不差；存档在会话列表里标出来、改它的方法回 `archived`、读的照常；重导是空操作；传上来的、本机路径两种都认；
//! 坏包、参数不对的拒。

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use miyu_kernel::id::SessionId;
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 在 `home` 里跑一轮，导出到 `home.work` 下的 `name`，交回会话编号和包的位置。
async fn exported(home: &Home, name: &str) -> (String, std::path::PathBuf) {
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "旧的一句话").await;
    home.until_turns(&session, 1).await;
    let out = home.work.join(name);
    let reply = client
        .call(
            "e1",
            "session.export",
            json!({"session": session, "format": "bundle", "to": {"path": out}}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    (session, out)
}

fn segment(home: &Home, session: &str) -> Vec<u8> {
    let dir = home
        .root
        .session_dir(&SessionId::parse(session).expect("合写法"));
    std::fs::read(dir.join("000000000001.jsonl")).expect("读得出")
}

async fn listed(client: &mut Client, session: &str) -> Value {
    let reply = client.call("l", "session.list", json!({})).await;
    reply["result"]["sessions"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|item| item["session"] == json!(session))
        .cloned()
        .unwrap_or_else(|| panic!("列得出 {session}：{reply}"))
}

#[tokio::test]
async fn an_archive_reads_like_any_session_but_cannot_be_driven() {
    let (source, target) = (Home::new(), Home::new());
    let (session, bundle) = exported(&source, "one.zip").await;
    let script = Script::new([Play::Says("不该开这一轮。")]);
    let mut client = Client::connect(target.core(&script));
    client.hello().await;
    let reply = client
        .call(
            "i1",
            "session.import",
            json!({"path": bundle, "archive": true}),
        )
        .await;
    assert_eq!(reply["result"]["sessions"], json!([session]), "{reply}");
    assert_eq!(reply["result"]["main"], json!(session));
    assert_eq!(
        segment(&target, &session),
        segment(&source, &session),
        "一个字节不差"
    );
    assert_eq!(listed(&mut client, &session).await["archived"], json!(true));
    // 命令编号别和原来那台机器上的撞：核心照命令编号去重，撞上的交回上一次的回应。
    let said = client
        .call(
            "s1",
            "session.send",
            json!({"session": session, "text": "接着说"}),
        )
        .await;
    assert_eq!(said["error"]["data"]["reason"], json!("archived"), "{said}");
    let revert = client
        .call("r1", "session.revert", json!({"session": session}))
        .await;
    assert_eq!(
        revert["error"]["data"]["reason"],
        json!("archived"),
        "{revert}"
    );
    let prompts = client
        .call("p1", "session.prompts", json!({"session": session}))
        .await;
    assert_eq!(
        prompts["result"]["prompts"][0]["text"],
        json!("旧的一句话"),
        "{prompts}"
    );
    let again = client
        .call(
            "i2",
            "session.import",
            json!({"path": bundle, "archive": true}),
        )
        .await;
    assert_eq!(again["result"]["sessions"], json!([]), "{again}");
    assert_eq!(again["result"]["present"], json!([session]));
}

#[tokio::test]
async fn an_uploaded_bundle_imports_as_a_live_session_unless_asked_otherwise() {
    let (source, target) = (Home::new(), Home::new());
    let (session, bundle) = exported(&source, "two.zip").await;
    let script = Script::new([Play::Says("接上了。")]);
    let mut client = Client::connect(target.core(&script));
    client.hello().await;
    let bytes = std::fs::read(&bundle).expect("读得出");
    let put = client
        .call(
            "b1",
            "blob.put",
            json!({"data": STANDARD.encode(&bytes), "name": "two.zip"}),
        )
        .await;
    let blob = put["result"]["blob"]
        .as_str()
        .unwrap_or_else(|| panic!("{put}"))
        .to_string();
    let reply = client
        .call("i1", "session.import", json!({"blob": blob}))
        .await;
    assert_eq!(reply["result"]["sessions"], json!([session]), "{reply}");
    assert!(
        listed(&mut client, &session)
            .await
            .get("archived")
            .is_none()
    );
    // 命令编号别和原来那台机器上的撞：核心照命令编号去重，撞上的交回上一次的回应、不再发。
    let said = client
        .call(
            "after-import",
            "session.send",
            json!({"session": session, "text": "换了台机器接着说"}),
        )
        .await;
    assert!(said.get("error").is_none(), "{said}");
    target.until_turns(&session, 2).await;
}

#[tokio::test]
async fn bad_bundles_and_bad_params_are_refused() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let junk = home.work.join("junk.zip");
    std::fs::write(&junk, "not a zip").expect("写得进");
    let cases = [
        (json!({"path": junk}), "bad_bundle"),
        (json!({"path": "relative.zip"}), "bad_params"),
        (json!({}), "bad_params"),
        (json!({"path": junk, "blob": "sha256:00"}), "bad_params"),
        (
            json!({"blob": "sha256:0000000000000000000000000000000000000000000000000000000000000000"}),
            "unknown_attachment",
        ),
    ];
    for (n, (params, reason)) in cases.into_iter().enumerate() {
        let reply = client
            .call(&format!("i{n}"), "session.import", params)
            .await;
        assert_eq!(reply["error"]["data"]["reason"], json!(reason), "{reply}");
    }
}
