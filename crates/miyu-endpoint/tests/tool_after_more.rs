//! 工具调用后的通知（施工 HK-2）：脚本里调的（`tool.nested`，core1 2026-11-07 照推荐定照发）带嵌套调用的编号、`tool.nested`
//! 的状态和 blob 的引用，不带 `blocks`；直接调的那一次（脚本工具本身）照它自己的表。场所会话测试用的核心也在这里。

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::extensions::*;
use crate::support::venues::BINDINGS;
use crate::support::*;
use crate::tool_after::{install_after, told};
use crate::tool_guard::{lines, results, talk, until_seen};

/// 照对应表、读好的清单起来的核心（同 `served_core`），工具目录有假的 `jot`、`peek`，模型照 `plays` 回。
pub(crate) fn served_with_jot(home: &Home, plays: Vec<Play>) -> Arc<miyu_endpoint::Core> {
    home.write("etc/config.toml", BINDINGS);
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let found = miyu_endpoint::packages::load(&resources, &home.root, Vec::new());
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let catalog = Catalog::new([
        Fake::new("jot", Access::Write, Act::Echo) as Arc<dyn Tool>,
        Fake::callable("peek", Access::Read, Act::Echo) as Arc<dyn Tool>,
    ])
    .expect("合写法");
    let script = Script::new(plays).titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, catalog, None, TOKEN)
            .with_extension_timing(quick())
            .with_config(config)
            .with_packages(found),
    );
    core.start_extensions();
    core
}

#[tokio::test]
async fn tools_called_from_scripts_are_told_too() {
    let home = Home::new();
    let program = Program::new();
    let path = install_after(&home, &program, ("acme-audit", "tools = [\"peek\"]"), None);
    let shell = Fake::commanding(
        "shell",
        Access::Read,
        Some(miyu_tool::RUN_IN_BACKGROUND),
        Act::Calls(vec![("peek", r#"{"q":1}"#)]),
    );
    let peek = Fake::callable("peek", Access::Read, Act::Echo);
    let catalog = Catalog::new([
        Arc::clone(&shell) as Arc<dyn Tool>,
        Arc::clone(&peek) as Arc<dyn Tool>,
    ])
    .expect("合写法");
    let script = Script::new([
        Play::calls(&[("shell", r#"{"command":"python3 x.py"}"#)]),
        Play::Says("好。"),
    ])
    .titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, catalog, None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    lines(&path, 2).await;
    let (_client, session) = talk(&core).await;
    home.until_turns(&session, 1).await;
    until_seen(&path, |text| text.contains("tool.after")).await;
    assert_eq!(peek.calls().len(), 1);

    let nested = logged(&home, &session)
        .into_iter()
        .find(|event| event["kind"] == json!("tool.nested"))
        .expect("记了嵌套调用");
    let told = told(&path);
    assert_eq!(told.len(), 1, "脚本工具本身不在表里，不发");
    let notice = &told[0];
    assert_eq!(notice["call_id"], nested["body"]["id"]);
    assert!(
        notice["call_id"]
            .as_str()
            .is_some_and(|id| id.contains('#')),
        "{notice}"
    );
    assert_eq!(notice["tool"], json!("peek"));
    assert_eq!(notice["access"], json!("read"));
    assert_eq!(notice["status"], json!("ok"));
    assert_eq!(
        notice["args"], nested["body"]["args"],
        "修正过的那一份，同事件流"
    );
    assert_eq!(notice["result"], nested["body"]["result"], "blob 的引用");
    assert_eq!(notice["bytes"], nested["body"]["bytes"]);
    assert!(notice["result"].is_string(), "{notice}");
    assert!(notice.get("blocks").is_none(), "不给 blocks：{notice}");
    assert_eq!(notice["by"], nested["by"]);
    // 直接调的那一次照它自己的表：表里没有 `shell`。
    assert_eq!(results(&home, &session).len(), 1);
    core.stop_extensions().await;
}
