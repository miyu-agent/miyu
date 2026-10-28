//! 扩展登记的旁路请求（施工 Q-3，设计 `43-旁路请求和档位池.md` 第三节第 2 到 4 条）：清单 `[[side]]` 登记了的，`model.call` 写
//! `side` 照档位从固定池取；池是空的用会话的模型（写了 `session` 的照那个会话，没写的照 `models.chat`）。只认自己包里登记的
//! 编号；`side` 和 `model`、`use` 不能同写，`session` 只跟着 `side`；人的连接没有包，写了 `side` 的 `bad_params`。`model.list`
//! 的 `side` 列出扩展登记的，编号是 `<包>/<编号>`。写了 `session` 的记账挂上那个会话的场所（施工 UC-2）。

use serde_json::{Value, json};

use crate::blob_session::replies;
use crate::support::extensions::*;
use crate::support::providers::{data, profiles, routed_serving, said};
use crate::support::*;
use miyu_http::testkit::Server;

/// 包 `bot` 的清单：替机主干活的扩展（不声明系统账号），登记 `judge`（档位 `lite`）、`deep`（档位 `flagship`）。另装一个不拉起
/// 的包 `other`，登记 `theirs`：`bot` 写它的编号不认。
fn install_bot(home: &Home, program: &str, steps: &[String]) {
    let args: Vec<String> = steps.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        "packages/bot/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Bot\" }}\n\n[command]\nname = \"bot\"\nprogram = \"{program}\"\nabout = {{ en = \"B\" }}\n\n[process]\nargs = [{}]\nstart = \"always\"\ncapabilities = {BRIDGE:?}\n\n[[side]]\nid = \"judge\"\ntier = \"lite\"\nname = {{ en = \"Reply judge\", zh = \"判断要不要回\" }}\nsummary = {{ en = \"Decides whether to reply.\", zh = \"群里有人说话时判断要不要回\" }}\n\n[[side]]\nid = \"deep\"\ntier = \"flagship\"\nname = {{ en = \"Deep\" }}\nsummary = {{ en = \"Thinks hard.\" }}\n",
            args.join(", ")
        ),
    );
    approve(home, "bot", &BRIDGE);
    home.write(
        "packages/other/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Other\" }}\n\n[command]\nname = \"other\"\nprogram = \"{program}\"\nabout = {{ en = \"O\" }}\n\n[process]\nstart = \"manual\"\n\n[[side]]\nid = \"theirs\"\ntier = \"lite\"\nname = {{ en = \"Theirs\" }}\nsummary = {{ en = \"Not yours.\" }}\n"
        ),
    );
}

/// 一家 `a` 在假服务器上；`models.chat` 是 `a/chat-m`，再接上 `rest`。
fn config(server: &Server, rest: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"a/chat-m\"\n\n{rest}",
        server.base_url
    )
}

/// 一次 `model.call`：问一句，另带 `extra` 里的几格。
fn asked(extra: &Value) -> String {
    let mut params = json!({"purpose": "judge", "messages": [{"role": "user", "text": "hi"}]});
    for (key, value) in extra.as_object().expect("是对象") {
        params[key] = value.clone();
    }
    format!("ask:model.call:{params}")
}

/// 假服务器收到的每一次请求的 `model`，照先后。
fn models(server: &Server) -> Vec<String> {
    server
        .received()
        .iter()
        .map(|request| {
            let body: Value = serde_json::from_slice(&request.body).expect("JSON");
            body["model"].as_str().unwrap_or_default().to_string()
        })
        .collect()
}

#[tokio::test]
async fn a_registered_side_request_takes_its_tier_pool_and_nothing_else_passes() {
    let server = Server::start(vec![said("yes")]).await;
    let home = Home::new();
    home.write(
        "config.toml",
        &config(&server, "[pools.lite]\nmodels = [\"a/lite-m\"]\n"),
    );
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let script = steps(&[
        &step,
        "hello",
        &asked(&json!({"side": "judge"})),
        &asked(&json!({"side": "nope"})),
        &asked(&json!({"side": "theirs"})),
        &asked(&json!({"side": "judge", "model": "a/chat-m"})),
        &asked(&json!({"side": "judge", "use": "chat"})),
        &asked(&json!({"session": "0192f3a0-0000-7000-8000-000000000001"})),
        "ask:model.list:{}",
        "wait",
    ]);
    install_bot(&home, &program.name(), &script);
    let core = routed_serving(&home, data(profiles(json!({}))));
    let read = replies(&path, 8).await;
    assert_eq!(read[1]["result"]["text"], "yes", "{:?}", read[1]);
    assert_eq!(models(&server), ["lite-m"], "照档位从 lite 池取");
    for (at, why) in [
        (2, "没登记的编号"),
        (3, "别的包登记的编号"),
        (4, "和 model 同写"),
        (5, "和 use 同写"),
        (6, "session 不跟着 side"),
    ] {
        assert_eq!(
            read[at]["error"]["data"]["reason"], "bad_params",
            "{why}：{:?}",
            read[at]
        );
    }
    let listed = read[7]["result"]["side"].as_array().expect("有 side");
    let ids: Vec<&str> = listed.iter().filter_map(|one| one["id"].as_str()).collect();
    assert_eq!(
        ids,
        [
            "bot/deep",
            "bot/judge",
            "core/title",
            "memory/organize",
            "other/theirs"
        ],
        "出厂的、扩展的一起照编号排"
    );
    let deep = listed
        .iter()
        .find(|one| one["id"] == "bot/deep")
        .expect("列出");
    assert_eq!(
        (&deep["tier"], &deep["resolved"]),
        (&json!("flagship"), &json!("session")),
        "flagship 池是空的"
    );
    let bot = listed
        .iter()
        .find(|one| one["id"] == "bot/judge")
        .unwrap_or_else(|| panic!("列出扩展登记的：{listed:?}"));
    assert_eq!(
        bot,
        &json!({"id": "bot/judge", "tier": "lite", "name": "Reply judge",
            "summary": "Decides whether to reply.", "resolved": "lite"})
    );
    // 人的连接没有包：写了 `side` 的不认。
    let mut client = Client::connect(core);
    client.hello().await;
    let human = client
        .call(
            "h",
            "model.call",
            json!({"purpose": "judge", "side": "judge",
            "messages": [{"role": "user", "text": "hi"}]}),
        )
        .await;
    assert_eq!(human["error"]["data"]["reason"], "bad_params", "{human}");
    assert_eq!(models(&server), ["lite-m"], "拒了的都没发");
}

#[tokio::test]
async fn an_empty_tier_pool_falls_to_the_session_model_or_models_chat() {
    let server = Server::start(vec![said("one"), said("two")]).await;
    let home = Home::new();
    home.write("config.toml", &config(&server, ""));
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let script = steps(&[
        &step,
        "hello",
        r#"ask:session.create:{"model":"a/sess-m"}"#,
        &asked(&json!({"side": "judge", "session": "{session}"})),
        &asked(&json!({"side": "judge"})),
        "ask:model.list:{}",
        "wait",
    ]);
    install_bot(&home, &program.name(), &script);
    let core = routed_serving(&home, data(profiles(json!({}))));
    let read = replies(&path, 5).await;
    assert!(read[1]["result"]["session"].is_string(), "{:?}", read[1]);
    assert_eq!(read[2]["result"]["text"], "one", "{:?}", read[2]);
    assert_eq!(read[3]["result"]["text"], "two", "{:?}", read[3]);
    assert_eq!(
        models(&server),
        ["sess-m", "chat-m"],
        "池是空的：写了会话的照它的模型，没写的照 models.chat"
    );
    let listed = read[4]["result"]["side"].as_array().expect("有 side");
    let bot = listed
        .iter()
        .find(|one| one["id"] == "bot/judge")
        .expect("列出");
    assert_eq!(bot["resolved"], "session", "{bot}");

    // 记账（施工 UC-2）：写了会话的挂上那个会话的场所，没写的不挂；照场所、用途分得出来。
    let mut owner = Client::connect(core);
    owner.hello().await;
    let split = owner
        .call("u", "usage.query", json!({"group": ["venue", "purpose"]}))
        .await;
    let mut rows: Vec<(Value, Value, Value)> = split["result"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("{split}"))
        .iter()
        .map(|row| {
            (
                row["venue"].clone(),
                row["purpose"].clone(),
                row["requests"].clone(),
            )
        })
        .collect();
    rows.sort_by_key(|row| row.0.to_string());
    assert_eq!(
        rows,
        [
            (json!("local"), json!("judge"), json!(1)),
            (Value::Null, json!("judge"), json!(1)),
        ],
        "{split}"
    );
}
