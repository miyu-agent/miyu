//! 扩展读 blob（施工 W-6 三补，`protocol.md` 的 `blob.get` 第 1 条）：不写 `session` 的只读得到它自己存的、经 `package.call`
//! 拿回来的（答的包自己拿得到的那几个）；写了 `session` 的，那个会话要引用过这个 blob，扩展是管理员也一样。以前不写 `session`
//! 的照管理员名下给、写了管理员的会话照属主放行，扩展知道哈希就读得走主人任何一个 blob。人的连接照旧。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use miyu_kernel::id::ContentHash;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 等到记下的回应有 `n` 行（`cwd:` 那一行不算），交回它们。
async fn replies(path: &Path, n: usize) -> Vec<Value> {
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
fn got(reply: &Value) -> Result<Vec<u8>, String> {
    match reply["result"]["data"].as_str() {
        Some(data) => Ok(STANDARD.decode(data).expect("是 base64")),
        None => Err(reply["error"]["data"]["reason"]
            .as_str()
            .unwrap_or_default()
            .to_string()),
    }
}

fn hash(bytes: &[u8]) -> String {
    ContentHash::of(bytes).as_str().to_string()
}

/// 不写 `session` 的读法。
fn get(blob: &str) -> String {
    format!(r#"ask:blob.get:{{"blob":"{blob}"}}"#)
}

/// 人的连接存一份。
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

#[tokio::test]
async fn an_extension_reads_only_blobs_it_stored_or_was_handed() {
    let home = Home::new();
    let gate = home.root.path().join("go");
    let pause = format!("pause:{}", gate.display());
    let (own, secret) = (&b"memes own"[..], &b"owner secret"[..]);
    let (own_hash, secret_hash) = (hash(own), hash(secret));
    let (memes, bridge) = (Program::new(), Program::new());
    let (memes_log, step) = record(&home, "memes");
    let store = format!(
        r#"ask:blob.put:{{"data":"{}","name":"m.png"}}"#,
        STANDARD.encode(own)
    );
    install_asking(
        &home,
        "memes",
        &memes.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &pause,
            &store,
            &get(&own_hash),
            &get(&secret_hash),
            &format!("give:{own_hash},{secret_hash}"),
            r#"ask:package.methods:{"methods":[{"name":"give"}]}"#,
            "serve",
        ]),
        &[],
    );
    let manifest = home
        .root
        .path()
        .join("home/alice/packages/memes/package.toml");
    let written = read(&manifest);
    std::fs::write(
        &manifest,
        format!("{written}\n[export]\nmethods = [\"give\"]\n"),
    )
    .expect("写得进");
    let ready = home.root.path().join("registered");
    let (bridge_log, step) = record(&home, "bridge");
    install_asking(
        &home,
        "bridge",
        &bridge.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &format!("pause:{}", ready.display()),
            &get(&own_hash),
            r#"ask:package.call:{"package":"memes","method":"give"}"#,
            &get(&own_hash),
            &get(&secret_hash),
            "wait",
        ]),
        &["packages.call"],
    );
    approve(&home, "bridge", &["packages.call"]);
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(core);
    client.hello().await;
    assert_eq!(put(&mut client, "p1", secret).await, secret_hash);
    std::fs::write(&gate, "").expect("写得进");

    let read_memes = replies(&memes_log, 5).await;
    assert_eq!(read_memes[1]["result"]["blob"], own_hash.as_str());
    assert_eq!(got(&read_memes[2]), Ok(own.to_vec()), "自己存的读得到");
    assert_eq!(
        got(&read_memes[3]),
        Err("unknown_blob".to_string()),
        "主人存的读不到"
    );
    assert!(read_memes[4].get("error").is_none(), "{read_memes:?}");
    std::fs::write(&ready, "").expect("写得进");

    let read_bridge = replies(&bridge_log, 5).await;
    assert_eq!(
        got(&read_bridge[1]),
        Err("unknown_blob".to_string()),
        "别的包存的，没拿到过读不到"
    );
    assert_eq!(
        read_bridge[2]["result"]["given"],
        json!([own_hash, secret_hash]),
        "{read_bridge:?}"
    );
    assert_eq!(
        got(&read_bridge[3]),
        Ok(own.to_vec()),
        "经 package.call 拿回来、答的包拿得到的读得到"
    );
    assert_eq!(
        got(&read_bridge[4]),
        Err("unknown_blob".to_string()),
        "答的包写进回应、自己拿不到的照样读不到"
    );

    // 每个带进程的包是自己的账号（施工 Y-5 中上）：memes 存的在它自己名下，转给桥的拷进了桥的名下；人的连接不写 `session` 的
    // 照自己名下读，包自己存的没交给人，读不到；人自己存的照旧读得到。
    let blobs = |account: &str| {
        let account = miyu_kernel::id::AccountId::parse(account).expect("合写法");
        miyu_store::blob::Blobs::new(home.root.blobs(&account))
    };
    let own_hash_id = miyu_kernel::id::ContentHash::parse(&own_hash).expect("合写法");
    assert!(
        blobs("memes").path(&own_hash_id).is_file(),
        "存在 memes 名下"
    );
    assert!(
        blobs("bridge").path(&own_hash_id).is_file(),
        "转过去的拷进了桥的名下"
    );
    let reply = client
        .call("g1", "blob.get", json!({"blob": own_hash}))
        .await;
    assert_eq!(got(&reply), Err("unknown_blob".to_string()), "{reply}");
    let reply = client
        .call("g2", "blob.get", json!({"blob": secret_hash}))
        .await;
    assert_eq!(got(&reply), Ok(secret.to_vec()), "{reply}");
}

#[tokio::test]
async fn an_extension_naming_a_session_reads_only_what_it_refers_to() {
    let home = Home::new();
    let script = Script::new([Play::Says("收到。")]);
    // 先起一个没有扩展的核心：主人开一个本机会话，带附件说一句；另存一份没发出去的。
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let local = client.create("c1", "~").await;
    let shown = put(&mut client, "p1", b"shown").await;
    let kept = put(&mut client, "p2", b"kept").await;
    let reply = client
        .call(
            "s1",
            "session.send",
            json!({"session": local, "text": "看这个",
                   "attachments": [{"blob": shown, "name": "a.txt", "media_type": "text/plain"}]}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&local, 1).await;
    drop(client);
    drop(core);

    // 装上一个没声明系统账号的扩展（它是管理员）再起来：照这个会话读两份。
    let program = Program::new();
    let (path, step) = record(&home, "plain");
    let ask = |blob: &str| format!(r#"ask:blob.get:{{"blob":"{blob}","session":"{local}"}}"#);
    install_asking(
        &home,
        "plain",
        &program.name(),
        "always",
        &steps(&[&step, "hello", &ask(&shown), &ask(&kept), "wait"]),
        &[],
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let read = replies(&path, 3).await;
    assert_eq!(got(&read[1]), Ok(b"shown".to_vec()), "会话引用过的读得到");
    assert_eq!(
        got(&read[2]),
        Err("unknown_blob".to_string()),
        "会话没引用过的读不到：{read:?}"
    );
    // 人的连接照旧：管理员照会话的属主读，没引用过的也读得到。
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client
        .call("g1", "blob.get", json!({"blob": kept, "session": local}))
        .await;
    assert_eq!(got(&reply), Ok(b"kept".to_vec()), "{reply}");
}
