//! 只剩系统配置一层（施工 SU-3 上，`docs/construction/SU-移除多用户.md` 第十五节）：`config.set` 不写 `layer` 的写进系统配置，
//! 写 `personal` 的参数不对；手改的密钥文件下一轮就用上。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_config::secret::Reference;
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 系统配置在数据根里的位置。
const CONFIG: &str = "config.toml";

fn read(home: &Home, relative: &str) -> Option<String> {
    std::fs::read_to_string(home.root.path().join(relative)).ok()
}

#[tokio::test]
async fn the_layer_can_be_left_out_and_personal_is_gone() {
    let home = Home::new();
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    let change = json!({"key": "ui.language", "input": "zh"});
    let gone = client
        .call(
            "c1",
            "config.set",
            json!({"layer": "personal", "changes": [change]}),
        )
        .await;
    assert_eq!(reason(&gone), Some("bad_params"), "{gone}");
    assert!(read(&home, CONFIG).is_none());
    let reply = client
        .call("c2", "config.set", json!({"changes": [change]}))
        .await;
    assert_eq!(
        reply["result"]["keys"]["ui.language"]["origin"]["layer"], "system",
        "{reply}"
    );
    assert!(
        read(&home, CONFIG)
            .unwrap_or_default()
            .ends_with("[ui]\nlanguage = \"zh\"\n")
    );
}

/// 手改密钥文件：监视看到了，换上的那一份推给会话，下一轮取得到新的 key。
#[tokio::test]
async fn a_hand_edited_secret_reaches_the_next_turn() {
    let home = Home::new();
    home.root.prepare_workspace().expect("建得了家目录");
    let script = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let core = home.core_configured(&script, None, &[]);
    let _watching = core.watch_config().expect("监视得了");
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let key = Reference::Secret("hand".to_string());
    assert!(script.configs()[0].secret(&key).is_none(), "还没写");

    home.write("secrets.toml", "hand = \"sk-FAKE-HAND\"\n");
    let mut n = 0;
    loop {
        n += 1;
        let listed = client
            .call(&format!("l{n}"), "secret.list", json!({}))
            .await;
        if listed["result"]["secrets"][0]["set"] == json!(true) {
            break;
        }
        assert!(n < 1000, "十秒内看到了手改：{listed}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    client.say("s2", &session, "hi").await;
    home.until_turns(&session, 2).await;
    assert!(
        script.configs()[1].secret(&key).is_some(),
        "下一轮用的是换上的那一份"
    );
}
