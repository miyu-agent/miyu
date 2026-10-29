//! `provider.test`（施工 8-11、8-11 五补，`docs/blueprint/models.md`「协议」、「怎么走」第七条第 4 条）：对本机回环上的假
//! 服务器试。列得出模型就算通、不发请求（`checked: list`）；写的模型不在列表里、列不出又不是认证错的才发一句（`checked:
//! request`，交 `first_token_ms`、收到第一段正文就停）；列不出、没写模型的要模型名（`need_model`）；配好的存列表、候选不存；
//! 认证失败交分类、状态、原话；推荐的模型；推不出的 `config`，引用取不到的照连接的语言说；参数不对、`unknown_provider`；配了
//! 温度的照带（施工 8-22）。

use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::providers::{core, data, profiles};
use crate::support::*;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_session::ModelData;

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";

/// 列模型的回应。
fn listing(models: &[&str]) -> Reply {
    let data: Vec<Value> = models.iter().map(|id| json!({"id": id})).collect();
    let body = json!({"object": "list", "data": data}).to_string();
    Reply::stream(vec![Piece::Bytes(body.into_bytes())])
}

/// 说了一段思考、一段正文，然后停住不动：不叫停就一直等。
fn first_words_then_stall() -> Reply {
    let chunk = |delta: Value| {
        let event = json!({"id": "c1", "object": "chat.completion.chunk", "model": "x",
            "choices": [{"index": 0, "delta": delta, "finish_reason": null}]});
        format!("data: {event}\n\n")
    };
    let text = [
        chunk(json!({"role": "assistant", "content": ""})),
        chunk(json!({"reasoning_content": "thinking"})),
        chunk(json!({"content": "OK"})),
    ]
    .concat();
    Reply::stream(vec![Piece::Bytes(text.into_bytes()), Piece::Stall])
}

/// 认证失败。
fn unauthorized() -> Reply {
    Reply::error(
        401,
        &[("Content-Type", "application/json")],
        r#"{"error":{"message":"Authentication Fails (no such user)","type":"authentication_error"}}"#,
    )
}

/// 照配置、环境、模型资料起核心，问一次 `provider.test`。
async fn test(home: &Home, env: &[(&str, &str)], data: Arc<ModelData>, params: Value) -> Value {
    let core = core(home, env, data);
    let mut client = Client::connect(core);
    client.hello().await;
    client.call("test-1", "provider.test", params).await
}

fn fresh() -> Arc<ModelData> {
    data(profiles(json!({})))
}

/// 发出去的那一句：一条 user，没有 system、没有工具。
fn asked(server: &Server, at: usize) -> Value {
    let request = &server.received()[at];
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/chat/completions");
    serde_json::from_slice(&request.body).expect("请求体是 JSON")
}

#[tokio::test]
async fn a_configured_provider_that_lists_works_without_a_request() {
    let server = Server::start(vec![listing(&[
        "deepseek-v4-pro",
        "deepseek-flash",
        "deepseek-v4-flash",
        "deepseek-flash",
        "a-tiny",
    ])])
    .await;
    let home = Home::new();
    home.write(
        "config.toml",
        &format!(
            "[providers.deepseek]\nbase_url = \"{}\"\nkey = {{ env = \"DEEPSEEK_API_KEY\" }}\n",
            server.base_url
        ),
    );
    let shared = fresh();
    let reply = test(
        &home,
        &[("DEEPSEEK_API_KEY", FAKE)],
        Arc::clone(&shared),
        json!({"provider": "deepseek"}),
    )
    .await;
    assert_eq!(
        reply["result"],
        json!({"ok": true, "checked": "list", "listed": "provider", "model": "deepseek-flash",
            "models": ["a-tiny", "deepseek-flash", "deepseek-v4-flash", "deepseek-v4-pro"]}),
        "照字节排、去重；推荐的是够格的里面发布最晚的，不是列表第一个；只列了模型的没有 first_token_ms"
    );
    let received = server.received();
    assert_eq!(received.len(), 1, "列到了就不发请求");
    assert_eq!(received[0].path, "/v1/models");
    let bearer = format!("Bearer {FAKE}");
    assert_eq!(received[0].header("authorization"), Some(bearer.as_str()));
    assert!(shared.list_fetched("deepseek").is_some(), "配好的存进列表");

    let listed = Server::start(vec![listing(&["deepseek-flash", "deepseek-v4-pro"])]).await;
    home.write(
        "config.toml",
        &format!("[providers.deepseek]\nbase_url = \"{}\"\n", listed.base_url),
    );
    let reply = test(
        &home,
        &[],
        fresh(),
        json!({"provider": "deepseek", "model": "deepseek-v4-pro"}),
    )
    .await;
    assert_eq!(
        reply["result"]["checked"], "list",
        "写的在列表里也不发：{reply}"
    );
    assert_eq!(reply["result"]["model"], "deepseek-v4-pro");
    assert_eq!(listed.received().len(), 1);
}

/// 写的模型不在列表里：拿它发一句，收到第一段正文就停（假服务器之后停住不动也照样成了）。
#[tokio::test]
async fn a_model_not_in_the_list_is_tried_with_one_request() {
    let server = Server::start(vec![
        listing(&["deepseek-v4-pro"]),
        first_words_then_stall(),
    ])
    .await;
    let home = Home::new();
    home.write(
        "config.toml",
        &format!(
            "[providers.deepseek]\nbase_url = \"{}\"\nkey = {{ env = \"DEEPSEEK_API_KEY\" }}\n",
            server.base_url
        ),
    );
    let reply = test(
        &home,
        &[("DEEPSEEK_API_KEY", FAKE)],
        fresh(),
        json!({"provider": "deepseek", "model": "deepseek-flash"}),
    )
    .await;
    let result = &reply["result"];
    assert_eq!(result["ok"], true, "{reply}");
    assert_eq!(result["checked"], "request");
    assert_eq!(result["listed"], "provider");
    assert_eq!(result["model"], "deepseek-flash");
    assert!(result["first_token_ms"].is_u64(), "{reply}");
    assert!(!reply.to_string().contains("FAKE"));
    let bearer = format!("Bearer {FAKE}");
    assert_eq!(
        server.received()[1].header("authorization"),
        Some(bearer.as_str())
    );
    let body = asked(&server, 1);
    assert_eq!(body["model"], "deepseek-flash");
    assert_eq!(
        body["messages"],
        json!([{"role": "user", "content": "Reply with OK."}]),
        "只有一条 user，那一句去掉了行尾的换行"
    );
    assert!(body.get("tools").is_none(), "没有工具面：{body}");
    assert!(body.get("temperature").is_none(), "没配温度的不带：{body}");
}

/// 发的那个模型配了温度的照带（施工 8-22，「怎么走」第十四条第 4 条），和会话真发时一样。
#[tokio::test]
async fn the_probe_takes_the_configured_temperature_of_the_model() {
    let server = Server::start(vec![
        listing(&["deepseek-v4-pro"]),
        first_words_then_stall(),
    ])
    .await;
    let home = Home::new();
    home.write(
        "config.toml",
        &format!(
            "[providers.deepseek]\nbase_url = \"{}\"\n\n[providers.deepseek.models.\"deepseek-flash\"]\ntemperature = 0.6\n",
            server.base_url
        ),
    );
    let reply = test(
        &home,
        &[],
        fresh(),
        json!({"provider": "deepseek", "model": "deepseek-flash"}),
    )
    .await;
    assert_eq!(reply["result"]["checked"], "request", "{reply}");
    assert_eq!(asked(&server, 1)["temperature"], 0.6);
}

/// 列不出（不是认证错）：模型照目录列；没写模型的照推荐挑一个发一句（施工 ST-1），写了的拿它发一句。
#[tokio::test]
async fn a_provider_that_cannot_list_is_tried_with_a_model_from_the_catalog() {
    let server = Server::start(vec![
        Reply::error(404, &[], "no such path"),
        first_words_then_stall(),
        Reply::error(404, &[], "no such path"),
        first_words_then_stall(),
    ])
    .await;
    let home = Home::new();
    let shared = fresh();
    let candidate = json!({"catalog": "deepseek", "base_url": server.base_url, "key": {"value": format!(" {FAKE}\n")}});
    let reply = test(
        &home,
        &[],
        Arc::clone(&shared),
        json!({"candidate": candidate}),
    )
    .await;
    let result = &reply["result"];
    assert_eq!(result["ok"], true, "{reply}");
    assert_eq!(result["checked"], "request", "没写模型的照目录挑一个发一句");
    assert_eq!(result["listed"], "catalog");
    let picked = result["model"].as_str().expect("挑了一个");
    assert!(
        result["models"]
            .as_array()
            .is_some_and(|models| models.iter().any(|model| model == picked)),
        "挑的是目录里的：{reply}"
    );
    assert_eq!(asked(&server, 1)["model"], picked, "发的就是挑的那个");

    let reply = test(
        &home,
        &[],
        Arc::clone(&shared),
        json!({"candidate": candidate, "model": "deepseek-flash"}),
    )
    .await;
    let result = &reply["result"];
    assert_eq!(result["ok"], true, "{reply}");
    assert_eq!(result["checked"], "request");
    assert_eq!(result["listed"], "catalog");
    assert_eq!(
        result["models"],
        json!([
            "deepseek-flash",
            "deepseek-v4-flash",
            "deepseek-v4-flash-vision-exp",
            "deepseek-v4-pro"
        ])
    );
    assert_eq!(result["model"], "deepseek-flash");
    let bearer = format!("Bearer {FAKE}");
    assert_eq!(
        server.received()[3].header("authorization"),
        Some(bearer.as_str()),
        "{{value}} 去掉前后空白"
    );
    assert!(shared.list_fetched("deepseek").is_none(), "候选不存列表");
    assert!(
        !home.root.path().join("secrets.toml").exists(),
        "也不存 key"
    );
}

#[tokio::test]
async fn a_candidate_is_never_kept_in_the_lists() {
    let server = Server::start(vec![listing(&["deepseek-flash"])]).await;
    let home = Home::new();
    let shared = fresh();
    let reply = test(
        &home,
        &[],
        Arc::clone(&shared),
        json!({"candidate": {"catalog": "deepseek", "base_url": server.base_url, "key": {"value": FAKE}}}),
    )
    .await;
    assert_eq!(reply["result"]["listed"], "provider", "{reply}");
    assert!(
        shared.list_fetched("deepseek").is_none(),
        "列到了也不存：还没配"
    );
}

/// 请求发了就报请求的：分类、状态、原话。
#[tokio::test]
async fn a_failed_request_says_class_status_and_message() {
    let server = Server::start(vec![Reply::error(404, &[], "no such path"), unauthorized()]).await;
    let home = Home::new();
    home.write("secrets.toml", &format!("mine = \"{FAKE}\"\n"));
    let reply = test(
        &home,
        &[],
        fresh(),
        json!({"candidate": {"driver": "openai-chat", "base_url": server.base_url,
            "key": {"secret": "mine"}}, "model": "deepseek-chat"}),
    )
    .await;
    let result = &reply["result"];
    assert_eq!(result["ok"], false, "{reply}");
    assert_eq!(result["stage"], "request", "请求发了就报请求的");
    assert!(result.get("need_model").is_none(), "{reply}");
    assert_eq!(result["error"]["class"], "auth");
    assert_eq!(result["error"]["status"], 401);
    assert!(
        result["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Authentication Fails")),
        "{reply}"
    );
    assert_eq!(
        asked(&server, 1)["model"],
        "deepseek-chat",
        "写了的照写的试"
    );
}

/// 列模型就认证失败的：停在 `list`，写没写模型都不发请求，也不要模型名；列到的是空的要模型名。
#[tokio::test]
async fn an_unauthorized_listing_stops_there() {
    let server = Server::start(vec![unauthorized(), unauthorized()]).await;
    let home = Home::new();
    let candidate =
        json!({"driver": "openai-chat", "base_url": server.base_url, "key": {"env": "RELAY_KEY"}});
    for params in [
        json!({"candidate": candidate}),
        json!({"candidate": candidate, "model": "deepseek-chat"}),
    ] {
        let reply = test(&home, &[("RELAY_KEY", FAKE)], fresh(), params).await;
        let result = &reply["result"];
        assert_eq!(result["stage"], "list", "{reply}");
        assert!(
            result.get("need_model").is_none(),
            "认证错的给模型名也没用：{reply}"
        );
        assert_eq!(result["error"]["class"], "auth");
        assert_eq!(result["error"]["status"], 401);
    }
    assert_eq!(server.received().len(), 2, "两次都只列了模型");

    let empty = Server::start(vec![listing(&[])]).await;
    let reply = test(
        &home,
        &[],
        fresh(),
        json!({"candidate": {"driver": "openai-chat", "base_url": empty.base_url}}),
    )
    .await;
    assert_eq!(
        reply["result"],
        json!({"ok": false, "stage": "list", "need_model": true,
            "error": {"class": "other", "message": "no models listed"}})
    );
}

/// 推不出的是 `config`；引用的密钥、环境变量取不到的照这个连接的语言说（和 `miyu check` 同一句，施工 8-11 五补），别的照旧是
/// 英文的一句。
#[tokio::test]
async fn what_cannot_be_worked_out_is_the_config_stage() {
    let home = Home::new();
    home.write(
        "config.toml",
        "[providers.deepseek]\nkey = { env = \"DEEPSEEK_API_KEY\" }\n\n[providers.gone]\nbase_url = \"http://127.0.0.1:9/v1\"\nkey = { secret = \"gone-key\" }\n\n[providers.far]\nbase_url = { env = \"FAR_URL\" }\n",
    );
    let reply = test(&home, &[], fresh(), json!({"candidate": {}})).await;
    assert_eq!(reply["result"]["stage"], "config", "{reply}");
    assert_eq!(reply["result"]["error"]["class"], "no_model");
    assert!(
        reply["result"]["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("needs base_url"))
    );
    for (provider, said) in [
        (
            "deepseek",
            "providers.deepseek.key 引用的环境变量 DEEPSEEK_API_KEY 核心起来时没有设",
        ),
        ("gone", "providers.gone.key 引用的密钥 gone-key 还没设"),
        (
            "far",
            "providers.far.base_url 引用的环境变量 FAR_URL 核心起来时没有设",
        ),
    ] {
        let reply = test(&home, &[], fresh(), json!({"provider": provider})).await;
        assert_eq!(
            reply["result"],
            json!({"ok": false, "stage": "config", "error": {"class": "no_model", "message": said}}),
            "{provider}"
        );
    }
    let reply = test(
        &home,
        &[],
        fresh(),
        json!({"candidate": {"catalog": "anthropic", "key": {"value": FAKE}}}),
    )
    .await;
    assert_eq!(reply["result"]["stage"], "config", "驱动还没有：{reply}");
}

#[tokio::test]
async fn wrong_params_and_unknown_providers_are_refused() {
    let home = Home::new();
    home.write(
        "config.toml",
        "[providers.deepseek]\nkey = { env = \"DEEPSEEK_API_KEY\" }\n",
    );
    for params in [
        json!({}),
        json!({"provider": "deepseek", "candidate": {}}),
        json!({"candidate": {"catalog": "deepseek", "headers": {}}}),
        json!({"candidate": {"key": {"secret": "a", "env": "B"}}}),
        json!({"candidate": {"key": {"other": "a"}}}),
        json!({"provider": "deepseek", "model": ""}),
        json!({"provider": 7}),
    ] {
        let reply = test(&home, &[], fresh(), params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    let reply = test(&home, &[], fresh(), json!({"provider": "nope"})).await;
    assert_eq!(reason(&reply), Some("unknown_provider"));
}

/// 施工 8-14：发的那一句照这个模型的驱动（Go 上的 MiniMax 走 `anthropic`），档案另配的头照固定的种子 `provider.test` 换；
/// 列模型不带它。模型没有驱动的是 `config`。写的模型不在列表里才发（施工 8-11 五补）。
#[tokio::test]
async fn the_probe_speaks_through_the_model_driver_with_the_profile_headers() {
    let anthropic = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/designs/samples/drivers/anthropic/streams/text.sse"),
    )
    .expect("样本读得到");
    let server = Server::start(vec![
        listing(&["gemini-3-pro"]),
        Reply::stream(vec![Piece::Bytes(anthropic)]),
        listing(&["minimax-m3"]),
    ])
    .await;
    let home = Home::new();
    home.write(
        "config.toml",
        &format!(
            "[providers.opencode-go]\nbase_url = \"{0}\"\nkey = {{ env = \"GO_KEY\" }}\n\n[providers.opencode]\nbase_url = \"{0}\"\nkey = {{ env = \"GO_KEY\" }}\n",
            server.base_url
        ),
    );
    let shared = data(profiles(
        json!({"opencode-go": {"headers": {"x-opencode-session": "ses_{session_digest}"}}}),
    ));
    let reply = test(
        &home,
        &[("GO_KEY", FAKE)],
        Arc::clone(&shared),
        json!({"provider": "opencode-go", "model": "minimax-m3"}),
    )
    .await;
    assert_eq!(reply["result"]["ok"], true, "{reply}");
    assert_eq!(reply["result"]["checked"], "request");
    let received = server.received();
    assert_eq!(received[0].header("x-opencode-session"), None, "列模型不带");
    assert_eq!(received[1].path, "/v1/messages");
    assert_eq!(received[1].header("x-api-key"), Some(FAKE));
    let wanted = format!("ses_{}", miyu_models::headers::digest("provider.test"));
    assert_eq!(
        received[1].header("x-opencode-session"),
        Some(wanted.as_str())
    );
    let reply = test(
        &home,
        &[("GO_KEY", FAKE)],
        shared,
        json!({"provider": "opencode", "model": "gemini-3-pro"}),
    )
    .await;
    let result = &reply["result"];
    assert_eq!(result["ok"], false, "{reply}");
    assert_eq!(result["stage"], "config");
    assert_eq!(result["error"]["class"], "no_model");
    assert_eq!(
        result["error"]["message"],
        r#"model "opencode/gemini-3-pro" needs driver "@ai-sdk/google", which is not available yet"#
    );
    assert_eq!(server.received().len(), 3, "没发");
}
