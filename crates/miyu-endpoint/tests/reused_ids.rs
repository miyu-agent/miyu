//! 同一个命令编号换了参数再来（施工 YC-7，`docs/blueprint/protocol.md`「同一个命令编号」）：拒 `id_reused`，不悄悄交回上一次
//! 的结果；参数一样的照旧去重；造会话只比 `cwd`、`dirs` 以外的；重启以后照老规矩。

use serde_json::json;

use crate::support::*;
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};

/// 给人看的路径：Windows 上去掉 `\\?\` 这个前缀，和回应里写的一样。
fn plain(path: &std::path::Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

#[tokio::test]
async fn a_resent_create_with_other_params_is_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let first = client.create("c-reuse", "~").await;
    // 只换了 `cwd`：交回原来那一个，`cwd` 照这一次的。
    let cwd = home.work.to_string_lossy().into_owned();
    let moved = client
        .call("c-reuse", "session.create", json!({"cwd": cwd}))
        .await;
    assert_eq!(moved["result"]["session"], json!(first), "{moved}");
    assert_eq!(moved["result"]["cwd"], json!(plain(&home.work)), "{moved}");
    // 换了别的：拒，什么都不造。
    let changed = client
        .call(
            "c-reuse",
            "session.create",
            json!({"cwd": "~", "memory": "off"}),
        )
        .await;
    assert_eq!(reason(&changed), Some("id_reused"), "{changed}");
    assert_eq!(
        changed["error"]["message"],
        json!("同一个请求编号换了参数，没有收。请换一个新的编号。")
    );
    assert_eq!(home.root.sessions().expect("列得出").len(), 1, "只造了一个");
}

#[tokio::test]
async fn a_resent_send_with_other_words_is_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let first = client.say("s-reuse", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let again = client.say("s-reuse", &session, "hi").await;
    assert_eq!(
        again["result"]["events"], first["result"]["events"],
        "{again}"
    );
    let changed = client.say("s-reuse", &session, "换了一句").await;
    assert_eq!(reason(&changed), Some("id_reused"), "{changed}");
    let said = home
        .log(&session)
        .iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .count();
    assert_eq!(said, 1, "只记了第一句");
}

/// 指纹只在内存里：重启以后从日志补回来的编号不比参数，照旧交回原来那一个。
#[tokio::test]
async fn after_a_restart_a_resent_create_keeps_the_old_rule() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let first = client.create("c-restart", "~").await;
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let changed = json!({"cwd": "~", "memory": "off"});
    assert_eq!(client.create_with("c-restart", changed).await, first);
}
