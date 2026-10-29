//! 工具说明的同名覆盖（施工 OV-1 上）：数据根的 `tools/<工具>.toml`（施工 SU-3 下），写了的格换上，开会话的
//! 工具面照它；写坏了的照出厂的，`check` 报；没写覆盖的工具字节不变。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_policy::{Snapshot, ToolEntry};
use miyu_store::blob::Blobs;

use crate::support::extensions::*;
use crate::support::*;

/// 会话 `session` 造的时候工具面上的 `name` 那一件。
fn tool(home: &Home, session: &str, name: &str) -> ToolEntry {
    let policy = home
        .log(session)
        .iter()
        .find_map(|event| match &event.body {
            Body::SessionCreated(created) => Some(created.policy.clone()),
            _ => None,
        })
        .expect("有快照");
    let bytes = Blobs::new(home.root.blobs())
        .get(&policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes)
        .expect("读得懂")
        .tools
        .into_iter()
        .find(|one| one.name == name)
        .unwrap_or_else(|| panic!("没有 {name}"))
}

fn parameters(entry: &ToolEntry) -> Value {
    serde_json::from_str(entry.parameters.get()).expect("读得懂")
}

#[tokio::test]
async fn overrides_change_the_face_and_bad_ones_are_reported() {
    let home = Home::new();
    let program = Program::new();
    let (log, record_step) = record(&home, "each");
    let one = |name: &str| {
        json!({"name": name, "description": "Shipped text.", "access": "read", "venues": ["local"],
               "input_schema": {"type": "object", "properties": {"text": {"type": "string", "description": "Shipped."}}}})
    };
    let tools = json!({"tools": [one("echo"), one("plain"), one("broken"), one("missing")]});
    install_providing(
        &home,
        "each",
        &program.name(),
        "always",
        &steps(&[
            &record_step,
            "hello",
            &format!("ask:provide:{tools}"),
            "serve",
        ]),
    );
    home.write(
        "tools/echo.toml",
        "[parameters.text]\ndescription = \"My param.\"\n",
    );
    home.write("tools/broken.toml", "description = \"读一个文件。\"\n");
    home.write(
        "tools/missing.toml",
        "[parameters.nope]\ndescription = \"No such.\"\n",
    );
    let core = core_with_settings(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    // 等它登记上工具。
    crate::support::mcp::until_line(&log, "\"tools\":4").await;
    let session = client.create("c1", "~").await;

    let echo = tool(&home, &session, "echo");
    assert_eq!(echo.description, "Shipped text.", "没写的照出厂的");
    assert_eq!(
        parameters(&echo)["properties"]["text"]["description"],
        "My param.",
        "写了的换上"
    );
    for shipped in ["broken", "missing"] {
        let entry = tool(&home, &session, shipped);
        assert_eq!(
            entry.description, "Shipped text.",
            "{shipped} 写坏了照出厂的"
        );
        assert_eq!(
            parameters(&entry)["properties"]["text"]["description"],
            "Shipped."
        );
    }
    let plain = tool(&home, &session, "plain");
    assert_eq!(
        plain.parameters.get(),
        tool(&home, &session, "broken").parameters.get(),
        "没写覆盖的参数格式字节照出厂"
    );

    let checked = client.call("k1", "check", json!({})).await;
    let mut files: Vec<String> = checked["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{checked}"))
        .iter()
        .filter(|one| one["code"] == "override_bad")
        .filter_map(|one| one["file"].as_str().map(str::to_string))
        .collect();
    files.sort();
    assert_eq!(files.len(), 2, "{checked}");
    assert!(
        files[0].ends_with("broken.toml") && files[1].ends_with("missing.toml"),
        "{files:?}"
    );
    core.stop_extensions().await;
}
