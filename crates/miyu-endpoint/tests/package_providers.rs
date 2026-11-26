//! 软件包自带的供应商（施工 PV-4）：真核心、真扩展进程走一遍。
//!
//! - 清单写了 `[providers.ag]` 的包装着、开着：配置里不用写，`model.list` 列得出这一家（名字照清单），模型照它驱动的
//!   `driver.models`；`config.get` 的来源是 `package`；切到它的模型说得出话；
//! - 人改这一家的名字：写进配置文件、带上 `package = "ag"`，名字的来源换成 `system`，没改的照旧是包给的；
//! - 关掉包：这一家没了，配置文件里盖在它上面的那一段也不算；
//! - 配置里原来就有自己的同编号供应商：人的优先，包带的不出现，`check` 提示 `provider_shadowed`；
//! - 不要密钥、头一次问的时候还没拉过列表（施工 ACP-9：`miyu acp` 撞到的）：这一次先答空的、在后台拉，拉到了推
//!   `models.changed`，再问列得出、都能用（不是 `no_key`），头照推送重问就选得到。

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::providers::{data, profiles, routed_serving_with};
use crate::support::*;

/// 装包 `ag`：扩展照 `driver:echo+steady+list` 答，声明驱动 `ag`、自带供应商 `ag`；批了 `model.driver`。
fn install(home: &Home, program: &str) {
    let args = steps(&["hello", "driver:echo+steady+list", "serve"]);
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        "opt/ag/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Ag\" }}\n\n[command]\nname = \"ag\"\nprogram = \"{program}\"\nabout = {{ en = \"A\" }}\n\n[process]\nargs = [{}]\nstart = \"always\"\ncapabilities = [\"model.driver\"]\n\n[[driver]]\nname = \"ag\"\n\n[providers.ag]\nname = {{ en = \"Antigravity\" }}\ndriver = \"ag/ag\"\nlocal = true\n",
            args.join(", ")
        ),
    );
    approve(home, "ag", &["model.driver"]);
}

async fn connected(home: &Home) -> (Client, std::sync::Arc<miyu_endpoint::Core>) {
    let core = routed_serving_with(home, &[], data(profiles(json!({}))));
    core.start_drivers();
    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    until_state(&mut client, "ag", |one| one["state"] == "running").await;
    (client, core)
}

/// `model.list` 里编号是 `id` 的那一家。
async fn provider(client: &mut Client, id: &str) -> Option<Value> {
    let reply = client
        .call("l", "model.list", json!({"refresh": true}))
        .await;
    reply["result"]["providers"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["id"] == id)
        .cloned()
}

async fn item(client: &mut Client, key: &str) -> Value {
    let reply = client.call("g", "config.get", json!({"keys": [key]})).await;
    reply["result"]["items"][key].clone()
}

#[tokio::test]
async fn a_package_provider_is_listed_usable_and_overridable() {
    let home = Home::new();
    let program = Program::new();
    install(&home, &program.name());
    let (mut client, core) = connected(&home).await;

    let listed = provider(&mut client, "ag")
        .await
        .expect("列得出包带的这一家");
    assert_eq!(listed["name"]["value"], "Antigravity", "{listed}");
    assert_eq!(listed["name"]["from"], "package", "{listed}");
    let models: Vec<&str> = listed["models"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .filter_map(|model| model["model"].as_str())
        .collect();
    assert_eq!(models, ["ag-lite", "ag-pro"], "照驱动的模型列表：{listed}");
    let driver = item(&mut client, "providers.ag.driver").await;
    assert_eq!(
        driver,
        json!({"origin": {"layer": "package", "package": "ag"}, "value": "ag/ag"})
    );

    let asked = json!({"purpose": "platform", "model": "ag/ag-pro", "messages": [{"role": "user", "text": "alpha"}]});
    let reply = client.call("c", "model.call", asked).await;
    assert_eq!(reply["result"]["text"], "echo: alpha", "{reply}");

    let set = client
        .call(
            "s",
            "config.set",
            json!({"changes": [{"key": "providers.ag.name", "value": "Mine"}]}),
        )
        .await;
    assert!(set.get("error").is_none(), "{set}");
    let written = std::fs::read_to_string(home.root.config_file()).expect("写了");
    assert!(
        written.contains("package = \"ag\""),
        "顺手写上是哪个包的：{written}"
    );
    let name = item(&mut client, "providers.ag.name").await;
    assert_eq!(name["value"], "Mine");
    assert_eq!(name["origin"]["layer"], "system", "{name}");
    let driver = item(&mut client, "providers.ag.driver").await;
    assert_eq!(
        driver["origin"]["layer"], "package",
        "没改的照包给的：{driver}"
    );

    let off = client
        .call("d", "package.disable", json!({"package": "ag"}))
        .await;
    assert!(off.get("error").is_none(), "{off}");
    assert_eq!(provider(&mut client, "ag").await, None, "关了就没有");
    assert_eq!(
        item(&mut client, "providers.ag.name").await["value"],
        Value::Null
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_person_provider_with_the_same_id_wins() {
    let home = Home::new();
    let program = Program::new();
    install(&home, &program.name());
    home.write(
        "etc/config.toml",
        "[providers.ag]\ndriver = \"openai-chat\"\nbase_url = \"https://mine.invalid\"\n",
    );
    let (mut client, core) = connected(&home).await;
    let driver = item(&mut client, "providers.ag.driver").await;
    assert_eq!(driver["value"], "openai-chat", "{driver}");
    assert_eq!(driver["origin"]["layer"], "system", "{driver}");
    assert_eq!(
        item(&mut client, "providers.ag.local").await["value"],
        Value::Null,
        "包带的整家不出现"
    );
    let reply = client.call("k", "check", json!({})).await;
    let codes: Vec<&str> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(|problem| problem["code"].as_str())
        .collect();
    assert!(codes.contains(&"provider_shadowed"), "{reply}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_keyless_package_provider_lists_its_models_once_fetched_in_the_background() {
    let home = Home::new();
    let program = Program::new();
    install(&home, &program.name());
    let (mut client, core) = connected(&home).await;
    let reply = client
        .call("s", "subscribe", json!({"stream": "config"}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let models = |reply: &Value| -> Vec<(String, Value)> {
        let found = reply["result"]["providers"]
            .as_array()
            .unwrap_or_else(|| panic!("{reply}"))
            .iter()
            .find(|one| one["id"] == "ag")
            .cloned()
            .unwrap_or_else(|| panic!("列得出包带的这一家：{reply}"));
        assert!(found.get("key").is_none(), "没写密钥：{found}");
        found["models"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|model| {
                (
                    model["ref"].as_str().unwrap_or_default().to_string(),
                    model["state"].clone(),
                )
            })
            .collect()
    };
    let first = client.call("l1", "model.list", json!({})).await;
    assert_eq!(models(&first), Vec::new(), "还没拉过：先答空的");
    let pushed = client.next().await.expect("拉到了就推");
    assert_eq!(pushed["method"], "models.changed", "{pushed}");
    assert_eq!(pushed["params"]["providers"], json!(["ag"]), "{pushed}");
    let again = client.call("l2", "model.list", json!({})).await;
    let ok = json!("ok");
    assert_eq!(
        models(&again),
        [
            ("ag/ag-lite".to_string(), ok.clone()),
            ("ag/ag-pro".to_string(), ok)
        ],
        "{again}"
    );
    core.stop_extensions().await;
}
