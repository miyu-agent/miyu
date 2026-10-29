//! 端口登记（施工 IM-5，接入通讯平台设计 06 第 4.6 节第 5 样）：扩展登记要听的端口，撞了的回 `port_taken`、不说是谁；同一个
//! 连接再登记照收；放开的、进程停了的别人能登记；人的连接看得到整张表；不是扩展的连接登记回 `not_an_extension`。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::*;

/// 等到 `path` 里记下的回应有 `n` 行（`cwd:` 那一行不算），交回它们。
async fn answers(path: &Path, n: usize) -> Vec<Value> {
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

/// 回应里拒绝的原因；没拒绝的是 `ok`。
fn why(reply: &Value) -> &str {
    reply["error"]["data"]["reason"].as_str().unwrap_or("ok")
}

#[tokio::test]
async fn ports_are_claimed_once_released_with_the_process_and_listed() {
    let home = Home::new();
    let (first, second) = (Program::new(), Program::new());
    let (mine, mine_step) = record(&home, "a");
    let (theirs, theirs_step) = record(&home, "b");
    let go = home.work.join("go");
    let again = home.work.join("again");
    install_providing(
        &home,
        "a",
        &first.name(),
        "always",
        &steps(&[
            &mine_step,
            "hello",
            r#"ask:port.claim:{"port":8000}"#,
            r#"ask:port.claim:{"port":8000}"#,
            r#"ask:port.claim:{"port":8001}"#,
            r#"ask:port.release:{"port":8001}"#,
            r#"ask:port.claim:{"port":8002}"#,
            "wait",
        ]),
    );
    install_providing(
        &home,
        "b",
        &second.name(),
        "always",
        &steps(&[
            &theirs_step,
            "hello",
            &format!("pause:{}", go.display()),
            r#"ask:port.claim:{"port":8000}"#,
            r#"ask:port.claim:{"port":8001}"#,
            r#"ask:port.claim:{"port":0}"#,
            r#"ask:port.release:{"port":8002}"#,
            &format!("pause:{}", again.display()),
            r#"ask:port.claim:{"port":8000}"#,
            "wait",
        ]),
    );
    let core = core_with_settings(&home, quick());
    let got = answers(&mine, 6).await;
    assert_eq!(
        [
            why(&got[1]),
            why(&got[2]),
            why(&got[3]),
            why(&got[4]),
            why(&got[5])
        ],
        ["ok"; 5],
        "{got:?}"
    );
    std::fs::write(&go, "").expect("写得进");
    let got = answers(&theirs, 5).await;
    assert_eq!(why(&got[1]), "port_taken", "别人登记着：{got:?}");
    assert!(got[1]["error"]["data"].get("package").is_none(), "不说是谁");
    assert_eq!(why(&got[2]), "ok", "放开了的别人能登记：{got:?}");
    assert_eq!(why(&got[3]), "bad_params", "{got:?}");
    assert_eq!(why(&got[4]), "ok", "放别人的不报错、也不动：{got:?}");

    // 人的连接看整张表；不是扩展的连接登记不了。
    let mut admin = Client::connect(Arc::clone(&core));
    admin.hello().await;
    let listed = admin.call("p1", "port.list", json!({})).await;
    assert_eq!(
        listed["result"],
        json!({"ports": [{"port": 8000, "package": "a"}, {"port": 8001, "package": "b"}, {"port": 8002, "package": "a"}]}),
        "{listed}"
    );
    let refused = admin.call("p2", "port.claim", json!({"port": 9000})).await;
    assert_eq!(why(&refused), "not_an_extension", "{refused}");

    // a 停了：它登记的跟着放开。
    let off = admin
        .call("p3", "extension.disable", json!({"package": "a"}))
        .await;
    assert!(off["result"].is_object(), "{off}");
    std::fs::write(&again, "").expect("写得进");
    let got = answers(&theirs, 6).await;
    assert_eq!(why(&got[5]), "ok", "进程停了别人能登记：{got:?}");
    // 它登记着、没人接手的 8002 也不列了。
    let listed = admin.call("p4", "port.list", json!({})).await;
    assert_eq!(
        listed["result"],
        json!({"ports": [{"port": 8000, "package": "b"}, {"port": 8001, "package": "b"}]}),
        "{listed}"
    );
    core.stop_extensions().await;
}
