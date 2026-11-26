//! 能选的模型跟着核心变（施工 ACP-9）：不要密钥的供应商（本机的服务，同扩展驱动的 `acp`、`antigravity`），列表是编辑器头一次
//! 开会话以后核心才在后台拉到的：拉到了编辑器收到新的整份配置项，之后开的会话也列得出。以前一个进程只问一次，一直列不出。

use serde_json::{Value, json};

use super::{Editor, kind};
use crate::support::Home;
use crate::support::onboarding::listing;
use crate::support::outside::Outside;
use miyu_http::testkit::Server;

/// 配置项里模型那一项能选的值，照先后。
fn models(options: &Value) -> Vec<String> {
    let model = options
        .as_array()
        .and_then(|options| options.iter().find(|option| option["id"] == "model"))
        .unwrap_or_else(|| panic!("有模型那一项：{options}"));
    model["options"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|group| group["options"].as_array().into_iter().flatten())
        .filter_map(|option| option["value"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn models_listed_later_reach_the_editor() {
    let server = Server::start(vec![listing(&["a", "b"]), listing(&["a", "b"])]).await;
    let config = format!(
        "[providers.near]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nlocal = true\n\n[models]\nchat = \"near/a\"\n",
        server.base_url
    );
    let home = Home::onboarding(&config, &[], json!({}));
    let work = Outside::new();
    let mut editor = Editor::new(&home).await;
    let (_, opened) = editor.open(&work.text()).await;
    // 头一次问的时候还没有列表：只有配置里点名的那一个（核心这时在后台拉）。
    let first = models(&opened["result"]["configOptions"]);
    assert_eq!(first, ["near/a"], "{opened}");
    // 拉到了：编辑器收到新的整份配置项。
    let (_, update) = editor
        .until(|message| {
            kind(message) == Some("config_option_update")
                && models(&message["params"]["update"]["configOptions"]).len() > 1
        })
        .await;
    let listed = models(&update["params"]["update"]["configOptions"]);
    assert_eq!(listed, ["near/a", "near/b"], "{update}");
    // 之后开的会话一开就列得出。
    let (_, again) = editor.open(&work.text()).await;
    assert_eq!(
        models(&again["result"]["configOptions"]),
        ["near/a", "near/b"],
        "{again}"
    );
}
