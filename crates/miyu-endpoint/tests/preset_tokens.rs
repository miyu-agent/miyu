//! 预设列表带 token 数（施工 DM-4，2026-11-09 发版前检查第 4 条）：每个读得出的预设带 `tokens`，是新会话照它开、不算人格的
//! 那几块（system 里人设、场所、群聊以外的，加上工具的说明和参数格式），和不写会话、无人格的 `session.context` 的
//! `estimated_total` 一样；
//! 写错了的不带。预览只备不造：列预设、预览上下文、预览工具面都不在 `workspace/` 下留会话目录。
//!
//! 预演不重读给模型看的字（施工 PERF-5）：文件没变的，再列一次预设、再预览一次上下文一份字都不读；改了提示词的覆盖、改了
//! 预设，下一次就是新的。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
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

/// 照 `resources` 起一份核心，别的同 `core_with_tools`：测试拿着同一个资源目录，数读了几次给模型看的字（施工 PERF-5）。
fn core_reading(home: &Home, resources: &ResourceRoot) -> Arc<Core> {
    let core = Core::new(
        home.root.clone(),
        resources.clone(),
        Arc::new(Script::new([])),
        tools(),
        None,
        TOKEN.to_string(),
    );
    Arc::new(
        core.with_sandbox(miyu_sandbox::Availability::Usable(PathBuf::from(
            "miyu-sandbox",
        ))),
    )
}

/// 终端界面打开一次、按一次 `/new` 发的那几样（施工 PERF-5）：列预设，无人格的、无人格照 `full` 的上下文各预览一次。
async fn open_new_session(client: &mut Client) -> Value {
    let list = client.call("l", "preset.list", json!({})).await;
    previewed(client, "full").await.expect("预览得出来");
    let got = client
        .call("c", "session.context", json!({"persona": null}))
        .await;
    assert!(got["result"]["estimated_total"].is_u64(), "{got}");
    list
}

#[tokio::test]
async fn previews_read_the_model_texts_only_once() {
    let home = Home::new();
    let resources = ResourceRoot::at(default_resources());
    let mut client = Client::connect(core_reading(&home, &resources));
    client.hello().await;
    let first = open_new_session(&mut client).await;
    let read = resources.texts_read();
    assert!(read > 50, "头一回照文件读：{read}");
    let again = open_new_session(&mut client).await;
    assert_eq!(resources.texts_read(), read, "文件没变的不再读");
    assert_eq!(first, again, "列出来的一样");
}

#[tokio::test]
async fn previews_follow_a_changed_prompt_override_and_preset_at_once() {
    let home = Home::new();
    let mut client = Client::connect(home.core_with_tools(&Script::new([]), tools(), TOKEN));
    client.hello().await;
    let before = client.call("l1", "preset.list", json!({})).await;
    let tokens = |list: &Value, preset: &str| {
        listed(list, preset)["tokens"]
            .as_u64()
            .unwrap_or_else(|| panic!("{preset} 带 tokens：{list}"))
    };
    // 改了提示词的覆盖：不等，下一次列就是新的。
    let rule = "Relative paths in links and embeds in a reply resolve against the session working \
                directory, and every other path in a reply is written out in full from the root.\n"
        .repeat(8);
    home.write("root/prompts/core/local-paths-rule.txt", &rule);
    let longer = client.call("l2", "preset.list", json!({})).await;
    assert!(
        tokens(&longer, "full") > tokens(&before, "full"),
        "覆盖长了，token 跟着多：{before} {longer}"
    );
    std::fs::remove_file(
        home.root
            .prompt_overrides()
            .join("core/local-paths-rule.txt"),
    )
    .expect("删得了覆盖");
    let back = client.call("l3", "preset.list", json!({})).await;
    assert_eq!(
        tokens(&back, "full"),
        tokens(&before, "full"),
        "删了覆盖照出厂的"
    );
    // 改了预设：你那一层的极简开发写错了，下一次列就报它、不带 token 数；改回来又带上。
    home.write("root/presets/minimal.toml", "[colors]\n");
    let broken = client.call("l4", "preset.list", json!({})).await;
    assert!(
        listed(&broken, "minimal").get("tokens").is_none()
            && listed(&broken, "minimal").get("problem").is_some(),
        "写错了的报出来：{broken}"
    );
    home.write(
        "root/presets/minimal.toml",
        "[preset]\nunlisted = \"off\"\n",
    );
    let fixed = client.call("l5", "preset.list", json!({})).await;
    assert_eq!(
        tokens(&fixed, "minimal"),
        tokens(&before, "minimal"),
        "改回来照旧：{fixed}"
    );
}
