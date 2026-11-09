//! 预设列表带 token 数（施工 DM-4，2026-11-09 发版前检查第 4 条）：每个读得出的预设带 `tokens`，是新会话照它开、不算人格的
//! 那几块（system 里人设、场所、群聊以外的，加上工具的说明和参数格式），和不写会话、无人格的 `session.context` 的
//! `estimated_total` 一样；
//! 写错了的不带。预览只备不造：列预设、预览上下文、预览工具面都不在 `workspace/` 下留会话目录。

use std::path::Path;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::*;

fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

fn listed<'a>(list: &'a Value, preset: &str) -> &'a Value {
    list["result"]["presets"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["preset"] == preset))
        .unwrap_or_else(|| panic!("有 {preset}：{list}"))
}

/// 无人格的新会话照 `preset` 开，各块估了加起来（没摊）：没有人格，就只有 system 和工具。
async fn previewed(client: &mut Client, preset: &str) -> Option<u64> {
    let got = client
        .call(
            "c",
            "session.context",
            json!({"preset": preset, "persona": null}),
        )
        .await;
    got["result"]["estimated_total"].as_u64()
}

#[tokio::test]
async fn each_preset_tells_its_tokens_like_a_fresh_session() {
    let home = Home::new();
    home.write("root/presets/broken.toml", "[colors]\n");
    let mut client = Client::connect(home.core_with_tools(&Script::new([]), tools(), TOKEN));
    client.hello().await;
    let list = client.call("l1", "preset.list", json!({})).await;
    for preset in ["full", "dev", "none"] {
        assert_eq!(
            listed(&list, preset)["tokens"].as_u64(),
            previewed(&mut client, preset).await,
            "{preset}：{list}"
        );
    }
    assert!(
        listed(&list, "full")["tokens"].as_u64() > listed(&list, "none")["tokens"].as_u64(),
        "工具多的多：{list}"
    );
    assert!(
        listed(&list, "broken").get("tokens").is_none(),
        "写错了的不带：{list}"
    );
}

#[tokio::test]
async fn previews_leave_no_session_folders() {
    let home = Home::new();
    let mut client = Client::connect(home.core_with_tools(&Script::new([]), tools(), TOKEN));
    client.hello().await;
    client.call("l1", "preset.list", json!({})).await;
    client.call("c1", "session.context", json!({})).await;
    client.call("t1", "session.tools", json!({})).await;
    let left: Vec<_> = std::fs::read_dir(home.root.workspace())
        .map(|dir| dir.flatten().map(|entry| entry.file_name()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "预览只备不造：{left:?}");
}
