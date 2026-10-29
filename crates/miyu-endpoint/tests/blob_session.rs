//! `blob.get` 带 `session`（施工 W-6 补，`protocol.md` 的 `blob.get` 第 1 条）：照那个会话找。桥（接 QQ 的扩展）读得到群会话里
//! 和主人私聊里引用过的 blob；只在正文里写着哈希的、没引用过的读不到，回
//! `unknown_blob`。人的连接读不到群里的；会话编号不合写法的 `bad_params`。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::id::ContentHash;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::venues::BINDINGS;
use crate::support::*;

/// 照对应表、读好的清单起来的核心，拉起开着的扩展（同 `bridge_venues.rs`）。
pub(crate) fn served_core(home: &Home, script: &Script) -> Arc<Core> {
    home.write("config.toml", BINDINGS);
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let found = miyu_endpoint::packages::load(&resources, &home.root);
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_config(config)
            .with_packages(found),
    );
    core.start_extensions();
    core
}

/// 装好的包 `bot` 是接 QQ 的桥（`[connection] platform`）。
pub(crate) fn on_qq(home: &Home) {
    let manifest = home.root.path().join("packages/bot/package.toml");
    let text = std::fs::read_to_string(&manifest).expect("装好了");
    if text.contains("[connection]") {
        return;
    }
    std::fs::write(
        &manifest,
        format!("{text}\n[connection]\nplatform = \"qq\"\n"),
    )
    .expect("写得进");
}

/// 等到记下的回应有 `n` 行（`cwd:` 那一行不算），交回它们。
pub(crate) async fn replies(path: &Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<Value> = read(path)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 读成的内容；拒了的是拒绝的原因。
pub(crate) fn got(reply: &Value) -> Result<Vec<u8>, String> {
    match reply["result"]["data"].as_str() {
        Some(data) => Ok(STANDARD.decode(data).expect("是 base64")),
        None => Err(reply["error"]["data"]["reason"]
            .as_str()
            .unwrap_or_default()
            .to_string()),
    }
}

/// 人的连接上传一份，交回它的哈希。
async fn put(client: &mut Client, id: &str, bytes: &[u8]) -> String {
    let reply = client
        .call(
            id,
            "blob.put",
            json!({"data": STANDARD.encode(bytes), "name": "a.txt"}),
        )
        .await;
    reply["result"]["blob"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"))
        .to_string()
}

/// 人的连接往会话里以外部身份 `who` 发一句，带附件 `blob`；正文里写着 `mention`。
async fn send(client: &mut Client, id: &str, session: &str, who: &str, blob: &str, mention: &str) {
    let reply = client
        .call(
            id,
            "session.send",
            json!({"session": session, "text": format!("看这个 {mention}"), "as": {"external": who},
                   "attachments": [{"blob": blob, "name": "a.txt", "media_type": "text/plain"}]}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
}

#[tokio::test]
async fn the_bridge_reads_blobs_its_sessions_refer_to_and_nothing_else() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let (group_ready, private_ready) = (
        home.work.join("group-ready"),
        home.work.join("private-ready"),
    );
    let (group_pic, private_pic, secret) = (b"group pic", b"private pic", b"secret");
    let [g, p, s] = [&group_pic[..], &private_pic[..], &secret[..]]
        .map(|bytes| ContentHash::of(bytes).as_str().to_string());
    let ask = |blob: &str| format!(r#"ask:blob.get:{{"blob":"{blob}","session":"{{session}}"}}"#);
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            r#"ask:venue.session:{"venue":"qq:group:1","kind":"group","persona":"engineer"}"#,
            // 群会话归桥（施工 Y-2 上起人的连接碰不到场所会话）：图由桥自己传、自己以外部身份发进群里。
            &format!(
                r#"ask:blob.put:{{"data":"{}","name":"a.txt"}}"#,
                STANDARD.encode(group_pic)
            ),
            &format!(
                r#"ask:session.send:{{"session":"{{session}}","text":"看这个 {s}","as":{{"external":"qq:20001"}},"attachments":[{{"blob":"{g}","name":"a.txt","media_type":"text/plain"}}]}}"#
            ),
            &format!("pause:{}", group_ready.display()),
            &ask(&g),
            &ask(&s),
            r#"ask:venue.session:{"venue":"qq:private:10001","kind":"private","peer":"qq:10001"}"#,
            &format!("pause:{}", private_ready.display()),
            &ask(&p),
            &ask(&s),
            r#"ask:blob.get:{"blob":"sha256:00","session":"{session}"}"#,
            "wait",
        ]),
        "always",
        true,
    );
    // 接 QQ 的桥：`venue.sessions` 照它列谁的会话，这里照它判读不读得到。
    on_qq(&home);
    let script = Script::new([Play::Says("嗯。"), Play::Says("在。")]);
    let core = served_core(&home, &script);
    let mut client = Client::connect(core);
    client.hello().await;
    for (n, bytes) in [&private_pic[..], &secret[..]].iter().enumerate() {
        put(&mut client, &format!("p{n}"), bytes).await;
    }

    let made = replies(&path, 2).await;
    let group = made[1]["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{made:?}"))
        .to_string();
    let sent = replies(&path, 4).await;
    assert!(sent[3].get("error").is_none(), "桥发进它的群：{sent:?}");
    // 桥传的存进存处（施工 SU-4 中），群会话也归主人。
    let hash = ContentHash::parse(&g).expect("合写法");
    assert!(
        miyu_store::blob::Blobs::new(home.root.blobs())
            .path(&hash)
            .is_file()
    );
    std::fs::write(&group_ready, "").expect("写得进");
    let read = replies(&path, 7).await;
    assert_eq!(
        got(&read[4]),
        Ok(group_pic.to_vec()),
        "群会话里引用过的读得到"
    );
    assert_eq!(
        got(&read[5]),
        Err("unknown_blob".to_string()),
        "只在正文里写着的读不到"
    );

    let private = read[6]["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{read:?}"))
        .to_string();
    send(&mut client, "s2", &private, "qq:10001", &p, &s).await;
    std::fs::write(&private_ready, "").expect("写得进");
    let read = replies(&path, 10).await;
    assert_eq!(
        got(&read[7]),
        Ok(private_pic.to_vec()),
        "主人的私聊里引用过的读得到"
    );
    assert_eq!(
        got(&read[8]),
        Err("unknown_blob".to_string()),
        "主人名下有、只在正文里写着哈希的读不到"
    );
    assert_eq!(got(&read[9]), Err("bad_params".to_string()), "哈希不合写法");

    // 人的连接读不到群里的（施工 Y-2 上：场所会话只在平台的后台看，人的头碰不到）；不合写法的
    // 会话编号 `bad_params`。
    let reply = client
        .call("g1", "blob.get", json!({"blob": g, "session": group}))
        .await;
    assert_eq!(got(&reply), Err("unknown_blob".to_string()), "{reply}");
    let reply = client
        .call("g2", "blob.get", json!({"blob": g, "session": "nope"}))
        .await;
    assert_eq!(got(&reply), Err("bad_params".to_string()), "{reply}");
}

/// 别的平台的会话、主人自己的本机会话，桥读不到：引用过的也不行（同 `venue.sessions` 不列给它）。主人自己照样读得到。
#[tokio::test]
async fn the_bridge_cannot_read_the_owners_own_sessions() {
    let home = Home::new();
    let script = Script::new([Play::Says("收到。")]);
    // 先起一个没有桥的核心：主人开一个本机会话，带附件说一句。
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let local = client.create("c1", "~").await;
    let blob = put(&mut client, "p1", b"mine").await;
    let reply = client
        .call(
            "s1",
            "session.send",
            json!({"session": local, "text": "看这个",
                   "attachments": [{"blob": blob, "name": "a.txt", "media_type": "text/plain"}]}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&local, 1).await;
    drop(client);
    drop(core);

    // 装上桥再起来：它照会话的编号去读那个附件。
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!(r#"ask:blob.get:{{"blob":"{blob}","session":"{local}"}}"#),
            "wait",
        ]),
        "always",
        true,
    );
    on_qq(&home);
    let core = served_core(&home, &Script::new([]));
    let read = replies(&path, 2).await;
    assert_eq!(got(&read[1]), Err("unknown_blob".to_string()), "{read:?}");
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client
        .call("g1", "blob.get", json!({"blob": blob, "session": local}))
        .await;
    assert_eq!(got(&reply), Ok(b"mine".to_vec()), "管理员读得到：{reply}");
}
