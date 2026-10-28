//! 软件之间互相调（施工 X-18，`package.call` 给扩展开）：声明了 `packages.call` 的扩展只调得到对方清单 `[export]` 里的方法，
//! 转过去的 `method.call` 带 `from`；没导出的 `not_exported`，没声明的 `needs_approval`，没有这个包的 `unknown_package`；人的
//! 连接照旧调得到后台页登记的全部，不带 `from`。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{}",
            read(path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 记下的回应（`cwd:` 那一行不算）。
fn replies(path: &Path) -> Vec<Value> {
    read(path)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// 调 `package` 的 `method`。
fn ask(package: &str, method: &str) -> String {
    format!(r#"ask:package.call:{{"package":"{package}","method":"{method}","params":{{"x":1}}}}"#)
}

#[tokio::test]
async fn an_extension_calls_only_what_another_exports() {
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
            r#"ask:package.methods:{"methods":[{"name":"add"},{"name":"secret"}]}"#,
            "serve",
        ]),
        &[],
    );
    let manifest = home.root.path().join("packages/memes/package.toml");
    let written = read(&manifest);
    std::fs::write(
        &manifest,
        format!("{written}\n[export]\nmethods = [\"add\"]\n"),
    )
    .expect("写得进");
    let (bridge_log, step) = record(&home, "bridge");
    install_asking(
        &home,
        "bridge",
        &bridge.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &pause,
            &ask("memes", "add"),
            &ask("memes", "secret"),
            &ask("nosuch", "add"),
            "wait",
        ]),
        &["packages.call"],
    );
    let (plain_log, step) = record(&home, "plain");
    install_asking(
        &home,
        "plain",
        &plain.name(),
        "always",
        &steps(&[&step, "hello", &pause, &ask("memes", "add"), "wait"]),
        &[],
    );
    approve(&home, "bridge", &["packages.call"]);
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    lines(&memes_log, 3).await;
    std::fs::write(&gate, "").expect("写得进");
    lines(&bridge_log, 5).await;
    lines(&plain_log, 3).await;

    let got = replies(&bridge_log);
    assert_eq!(
        got[1]["result"],
        json!({"answered": "add"}),
        "导出的调得到：{got:?}"
    );
    assert_eq!(got[2]["error"]["data"]["reason"], "not_exported", "{got:?}");
    assert_eq!(got[2]["error"]["data"]["method"], "secret");
    assert_eq!(
        got[3]["error"]["data"]["reason"], "unknown_package",
        "{got:?}"
    );
    let plain = replies(&plain_log);
    assert_eq!(
        plain[1]["error"]["data"]["reason"], "needs_approval",
        "没声明的：{plain:?}"
    );
    assert_eq!(
        plain[1]["error"]["data"]["capabilities"],
        json!(["packages.call"])
    );
    let asked = read(&memes_log);
    assert!(
        asked.contains(r#""from":"bridge""#),
        "转过去带 from：{asked}"
    );

    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let person = client
        .call(
            "p1",
            "package.call",
            json!({"package": "memes", "method": "secret"}),
        )
        .await;
    assert_eq!(
        person["result"],
        json!({"answered": "secret"}),
        "人照旧调得到全部：{person}"
    );
    let last = read(&memes_log)
        .lines()
        .last()
        .unwrap_or_default()
        .to_string();
    assert!(
        last.contains("secret") && !last.contains("\"from\""),
        "人的不带 from：{last}"
    );
    core.stop_extensions().await;
}
