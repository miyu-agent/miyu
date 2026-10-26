//! 供应商自用、共用两份都能用（施工 PL-1，2026-10-24 项目主人定）：同一个编号系统配置、个人设置都配了，`model.list` 列两份
//! （`relay` 自用、`relay@system` 共用，带 `layer`），各用各的 key；`config.get` 不列派生出来的那份。

use serde_json::{Value, json};

use crate::support::providers::{data, profiles, routed, said};
use crate::support::*;
use miyu_http::testkit::Server;

const SHARED: &str = "sk-FAKE-SHARED-0001";
const MINE: &str = "sk-FAKE-MINE-0002";

fn bearer(server: &Server, n: usize) -> Option<String> {
    server.received()[n]
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.clone())
}

#[tokio::test]
async fn mine_and_shared_copies_each_use_their_own_key() {
    let server = Server::start(vec![said("自用。"), said("共用。")]).await;
    let home = Home::new();
    home.write(
        "system/config.toml",
        &format!(
            "[providers.relay]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ secret = \"relay\" }}\n",
            server.base_url
        ),
    );
    home.write("system/secrets.toml", &format!("relay = \"{SHARED}\"\n"));
    home.write(
        "home/alice/settings.toml",
        "[providers.relay]\nkey = { secret = \"mine\" }\n",
    );
    // 自己也存了一个同名的 relay：共用那份照样只用系统那一份的。
    home.write(
        "home/alice/secrets.toml",
        &format!("mine = \"{MINE}\"\nrelay = \"sk-FAKE-WRONG-0003\"\n"),
    );
    let mut client = Client::connect(routed(&home, &[], data(profiles(json!({})))));
    client.hello().await;

    // 先调：列模型会在后台去拉清单，把假服务器备好的回答用掉。
    for (n, model, key) in [(0, "relay/m", MINE), (1, "relay@system/m", SHARED)] {
        let reply = client
            .call(
                "c",
                "model.call",
                json!({"purpose": "platform", "model": model,
                    "messages": [{"role": "user", "text": "hi"}]}),
            )
            .await;
        assert!(reply.get("result").is_some(), "{model}: {reply}");
        assert_eq!(bearer(&server, n), Some(format!("Bearer {key}")), "{model}");
    }

    let listed = client.call("l", "model.list", json!({})).await;
    let layers: Vec<(Value, Value)> = listed["result"]["providers"]
        .as_array()
        .expect("{listed}")
        .iter()
        .map(|row| (row["id"].clone(), row["layer"].clone()))
        .collect();
    assert_eq!(
        layers,
        [
            (json!("relay"), json!("personal")),
            (json!("relay@system"), json!("system")),
        ],
        "{listed}"
    );

    let names: Vec<&Value> = listed["result"]["providers"]
        .as_array()
        .expect("{listed}")
        .iter()
        .map(|row| &row["name"]["value"])
        .collect();
    assert_eq!(
        names,
        [&json!("relay"), &json!("relay")],
        "共用那份照原编号叫"
    );

    let got = client.call("g", "config.get", json!({})).await;
    assert!(
        !got.to_string().contains("@system"),
        "派生的那份不进配置：{got}"
    );
}

/// 选过共用那份以后自用那份删掉了（施工 PL-1 修，网页 2026-10-26 实测）：只有系统配置写了 `relay`，`relay@system/m` 指的就是它，
/// 不报 `bad_reference`，照样用共用的 key；最终值里写成 `relay/m`。
#[tokio::test]
async fn a_shared_reference_still_works_after_my_copy_is_gone() {
    let server = Server::start(vec![said("共用。")]).await;
    let home = Home::new();
    home.write(
        "system/config.toml",
        &format!(
            "[providers.relay]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ secret = \"relay\" }}\n",
            server.base_url
        ),
    );
    home.write("system/secrets.toml", &format!("relay = \"{SHARED}\"\n"));
    home.write(
        "home/alice/settings.toml",
        "[models]\nchat = \"relay@system/m\"\n",
    );
    let mut client = Client::connect(routed(&home, &[], data(profiles(json!({})))));
    let hello = client.hello().await;
    assert!(
        hello["result"].get("config_errors").is_none(),
        "不算 bad_reference：{hello}"
    );
    let reply = client
        .call(
            "c",
            "model.call",
            json!({"purpose": "platform", "model": "relay@system/m",
                "messages": [{"role": "user", "text": "hi"}]}),
        )
        .await;
    assert!(reply.get("result").is_some(), "{reply}");
    assert_eq!(bearer(&server, 0), Some(format!("Bearer {SHARED}")));
    let listed = client.call("l", "model.list", json!({})).await;
    assert_eq!(listed["result"]["uses"]["chat"], "relay/m", "{listed}");
}
