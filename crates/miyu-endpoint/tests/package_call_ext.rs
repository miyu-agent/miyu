//! 扩展照包编号只调得到自己（施工 BUS-5，设计 `67-总线.md` B5；原来的施工 X-18 让声明了 `packages.call` 的扩展照包编号调
//! 别的包导出的方法）：扩展的 `package.call` 写了别的包的编号 `own_package_only`，那个包在不在都一样；跨包走 `bus.call`，转过去
//! 的 `method.call` 带 `from`。调自己照旧：要声明了 `packages.call`，只调得到自己 `[export]` 里的。没声明的 `needs_approval`。人
//! 的连接照旧照包编号调得到登记了的全部（导出的、没导出的），不带 `from`。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 记下的回应（`cwd:` 那一行、核心发来的请求不算）。
fn replies(path: &Path) -> Vec<Value> {
    read(path)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|line| line.get("method").is_none())
        .collect()
}

/// 等到 `path` 里记下了 `n` 条回应。
async fn answered(path: &Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got = replies(path);
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 条：{}",
            read(path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 照包编号调 `package` 的 `method`。
fn ask(package: &str, method: &str) -> String {
    format!(r#"ask:package.call:{{"package":"{package}","method":"{method}","params":{{"x":1}}}}"#)
}

/// 往包 `id` 的清单后面接 `[export]`。
fn export(home: &Home, id: &str, lines: &str) {
    let manifest = home.root.packages().join(format!("{id}/package.toml"));
    let written = read(&manifest);
    std::fs::write(&manifest, format!("{written}\n[export]\n{lines}\n")).expect("写得进");
}

fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or_default()
}

#[tokio::test]
async fn an_extension_calls_only_itself_by_package_id() {
    let home = Home::new();
    let gate = home.root.path().join("go");
    let pause = format!("pause:{}", gate.display());
    let (memes, bridge, plain) = (Program::new(), Program::new(), Program::new());
    let (memes_log, step) = record(&home, "memes");
    install_asking(
        &home,
        "memes",
        &memes.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            r#"ask:package.methods:{"methods":[{"name":"pics.add"},{"name":"secret"}]}"#,
            "serve",
        ]),
        &[],
    );
    export(
        &home,
        "memes",
        "provides = [\"pics\"]\nmethods = [\"pics.add\"]",
    );
    let (bridge_log, step) = record(&home, "bridge");
    install_asking(
        &home,
        "bridge",
        &bridge.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            r#"ask:package.methods:{"methods":[{"name":"relay.echo"},{"name":"hidden"}]}"#,
            &pause,
            &ask("memes", "pics.add"),
            r#"ask:bus.call:{"name":"pics","method":"add","params":{"x":1}}"#,
            &ask("bridge", "relay.echo"),
            &ask("bridge", "hidden"),
            &ask("memes", "secret"),
            &ask("nosuch", "add"),
            "wait",
        ]),
        &["packages.call"],
    );
    export(
        &home,
        "bridge",
        "provides = [\"relay\"]\nmethods = [\"relay.echo\"]",
    );
    let (plain_log, step) = record(&home, "plain");
    install_asking(
        &home,
        "plain",
        &plain.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &pause,
            &ask("memes", "pics.add"),
            &ask("plain", "x"),
            "wait",
        ]),
        &[],
    );
    approve(&home, "bridge", &["packages.call"]);
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    answered(&memes_log, 2).await;
    answered(&bridge_log, 2).await;
    std::fs::write(&gate, "").expect("写得进");
    let got = answered(&bridge_log, 8).await;
    let plain = answered(&plain_log, 3).await;

    assert_eq!(
        reason(&got[2]),
        "own_package_only",
        "照包编号调别的包导出的方法：拒，改用 bus.call：{got:?}"
    );
    assert_eq!(got[2]["error"]["data"]["package"], "memes", "{got:?}");
    assert_eq!(
        got[3]["result"],
        json!({"answered": "pics.add"}),
        "同一个方法照名字调得到：{got:?}"
    );
    assert_eq!(
        got[4]["result"],
        json!({"answered": "relay.echo"}),
        "照包编号调自己导出的照旧：{got:?}"
    );
    assert_eq!(
        (reason(&got[5]), &got[5]["error"]["data"]["method"]),
        ("not_exported", &json!("hidden")),
        "调自己没导出的照旧拒：{got:?}"
    );
    assert_eq!(
        reason(&got[6]),
        "own_package_only",
        "别的包没导出的也是这一句，不说它导没导出：{got:?}"
    );
    assert_eq!(
        reason(&got[7]),
        "own_package_only",
        "没有的包也是这一句，不说它在不在：{got:?}"
    );
    assert_eq!(got[7]["error"]["data"]["package"], "nosuch", "{got:?}");
    assert_eq!(reason(&plain[1]), "needs_approval", "没声明的：{plain:?}");
    assert_eq!(
        plain[1]["error"]["data"]["capabilities"],
        json!(["packages.call"])
    );
    assert_eq!(
        reason(&plain[2]),
        "needs_approval",
        "没声明的调自己也要批：{plain:?}"
    );
    let asked = read(&memes_log);
    assert_eq!(
        asked.matches(r#""method":"method.call""#).count(),
        1,
        "被拒的那几次没有转过去：{asked}"
    );
    assert!(
        asked.contains(r#""from":"bridge""#),
        "经总线转过去带 from：{asked}"
    );
    let own = read(&bridge_log);
    assert!(
        own.contains(r#""from":"bridge""#) && own.contains("relay.echo"),
        "调自己的转回来也带 from：{own}"
    );

    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    for (id, method) in [("p1", "secret"), ("p2", "pics.add")] {
        let person = client
            .call(
                id,
                "package.call",
                json!({"package": "memes", "method": method}),
            )
            .await;
        assert_eq!(
            person["result"],
            json!({"answered": method}),
            "人照旧照包编号调得到登记了的全部：{person}"
        );
        let last = read(&memes_log)
            .lines()
            .last()
            .unwrap_or_default()
            .to_string();
        assert!(
            last.contains(method) && !last.contains("\"from\""),
            "人的不带 from：{last}"
        );
    }
    core.stop_extensions().await;
}
