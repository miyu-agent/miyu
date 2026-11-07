//! 供应商写了扩展提供的驱动（施工 MD-1、MD-2，设计 66 第四、五、八、九节）：
//!
//! - 没装通道的（测试、第一次接入）一律当用不了，绝不拿主人给扩展配的 key、地址去走内置的驱动：单个模型当场 `no_model`、一个请求
//!   都不发；池里的照现成的规矩跳过、换下一个。写了没装的（配置记警告 `unknown_driver`、值照留）也一样。
//! - 装了通道（假端口）：回复照交来的增量写，`model.called` 记这一家、请求的哈希、扩展报的 `sent`；交过去的 key 只有这一家的，
//!   别家的 key 一次都不出现；`key = "none"` 的写了也不交，`required` 的没写当 `no_model`；驱动不在的当场 `no_model`，池里换下一个。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::support::Home;
use crate::support::routing::{called, configs_with_unknown_drivers, hellos, routes, turn};
use miyu_config::package::DriverKey;
use miyu_config::secret::Reference;
use miyu_http::testkit::Server;
use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, ErrorClass};
use miyu_models::provider::DriverRef;
use miyu_session::{Declared, DriverPending, DriverPort, NotYet};

/// 一家 `ag`：驱动写成扩展的 `driver`，地址指着假服务器 `base_url`，配了 key。
fn extension(driver: &str, base_url: &str) -> String {
    format!(
        "[providers.ag]\ndriver = \"{driver}\"\nbase_url = \"{base_url}\"\nkey = {{ env = \"AG_KEY\" }}\n\n"
    )
}

#[tokio::test]
async fn an_extension_driver_never_falls_back_to_a_built_in_one() {
    let server = Server::start(hellos(2)).await;
    for driver in ["pi-antigravity/antigravity", "gone/drv"] {
        let mut home = Home::new();
        home.configs = configs_with_unknown_drivers(
            &format!(
                "{}[models]\nchat = \"ag/m\"\n",
                extension(driver, &server.base_url)
            ),
            &[],
        );
        let handle = home
            .create(&routes(serde_json::json!({}), Duration::from_secs(60)))
            .await;
        turn(&handle, "cmd-1").await;
        let calls = called(&home, &handle);
        assert_eq!(calls.len(), 1, "{driver}");
        let error = calls[0].error.as_ref().expect("出错");
        assert_eq!(error.class, ErrorClass::NoModel, "{driver}");
        assert_eq!(
            error.message,
            format!(
                "driver \"{driver}\" of provider \"ag\" is provided by an extension, which isn't connected yet"
            )
        );
        assert_eq!(calls[0].endpoint, None, "没发出去");
    }
    assert!(
        server.received().is_empty(),
        "一个请求都没收到：没退回内置驱动发"
    );
}

#[tokio::test]
async fn in_a_pool_the_extension_member_is_skipped() {
    let ag = Server::start(hellos(1)).await;
    let other = Server::start(hellos(1)).await;
    let mut home = Home::new();
    home.configs = configs_with_unknown_drivers(
        &format!(
            "{}[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"@p\"\n\n[pools.p]\nmodels = [\"ag/m\", \"b/m\"]\n",
            extension("gone/drv", &ag.base_url),
            other.base_url
        ),
        &[],
    );
    let handle = home
        .create(&routes(serde_json::json!({}), Duration::from_secs(60)))
        .await;
    turn(&handle, "cmd-1").await;
    let calls = called(&home, &handle);
    assert!(
        calls.last().is_some_and(|call| call.error.is_none()),
        "{calls:?}"
    );
    assert!(ag.received().is_empty(), "扩展那一家一个请求都没收到");
    assert_eq!(other.received().len(), 1, "换到了下一个成员");
}

/// 假端口：驱动在不在、声明了什么照 `present`；每次调用记下参数，交一段字 `hi from ag`，回答带用量和 `sent`。
#[derive(Debug)]
struct Fake {
    present: Result<Declared, String>,
    params: Mutex<Vec<Value>>,
}

impl Fake {
    fn new(present: Result<Declared, String>) -> Arc<Fake> {
        Arc::new(Fake {
            present,
            params: Mutex::new(Vec::new()),
        })
    }

    fn params(&self) -> Vec<Value> {
        self.params.lock().expect("锁").clone()
    }
}

impl DriverPort for Fake {
    fn present(&self, driver: &DriverRef) -> Result<Declared, String> {
        assert_eq!(driver.to_string(), "pi/ag");
        self.present.clone()
    }

    fn call(
        &self,
        _: &DriverRef,
        _: &str,
        params: String,
        deltas: mpsc::UnboundedSender<Value>,
    ) -> Result<DriverPending, NotYet> {
        self.params
            .lock()
            .expect("锁")
            .push(serde_json::from_str(&params).expect("参数是 JSON"));
        drop(deltas.send(json!([
            {"start": {"index": 0, "kind": "text"}},
            {"text": {"index": 0, "text": "hi from ag"}},
            {"end": {"index": 0}}
        ])));
        Ok(Box::pin(async {
            Ok(json!({
                "usage": {"uncached": 7, "cache_read": 0, "cache_write": 0, "output": 3},
                "sent": format!("sha256:{}", "cd".repeat(32))
            }))
        }))
    }
}

fn declared(key: DriverKey) -> Result<Declared, String> {
    Ok(Declared { key, images: false })
}

/// 两家：`ag` 走扩展驱动 `pi/ag`（`key` 照写，没地址），`b` 走内置的、配了别的 key；`chat` 照写。
fn two(ag_key: bool, chat: &str, b_url: &str) -> String {
    let key = match ag_key {
        true => "key = { env = \"AG_KEY\" }\n",
        false => "",
    };
    format!(
        "[providers.ag]\ndriver = \"pi/ag\"\n{key}\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{b_url}\"\nkey = {{ env = \"B_KEY\" }}\n\n[models]\nchat = \"{chat}\"\n\n[pools.p]\nmodels = [\"ag/m\", \"b/m\"]\n"
    )
}

/// 照 `source` 起一个会话、说一轮，端口是 `fake`：交回会话的家、句柄。
async fn talk(fake: &Arc<Fake>, source: &str) -> (Home, miyu_session::Handle) {
    let mut home = Home::new();
    home.configs = configs_with_unknown_drivers(
        source,
        &[
            (Reference::Env("AG_KEY".to_string()), "sk-ag"),
            (Reference::Env("B_KEY".to_string()), "sk-b-secret"),
        ],
    );
    let routes = routes(json!({}), Duration::from_secs(60));
    routes
        .data
        .on_drivers(Arc::clone(fake) as Arc<dyn DriverPort>);
    let handle = home.create(&routes).await;
    turn(&handle, "cmd-1").await;
    (home, handle)
}

#[tokio::test]
async fn the_reply_comes_from_the_extension_and_only_its_key_is_handed_over() {
    let unused = Server::start(hellos(1)).await;
    let fake = Fake::new(declared(DriverKey::Optional));
    let (home, handle) = talk(&fake, &two(true, "ag/m", &unused.base_url)).await;
    let calls = called(&home, &handle);
    let call = calls.last().expect("有一次");
    assert!(call.error.is_none(), "{call:?}");
    assert_eq!(call.endpoint.as_ref().map(|id| id.as_str()), Some("ag"));
    assert!(call.request.is_some(), "请求的哈希照记");
    // 主请求、起标题这类旁路请求都记扩展报的 `sent`。
    for each in &calls {
        assert_eq!(
            each.sent.as_ref().map(ToString::to_string),
            Some(format!("sha256:{}", "cd".repeat(32))),
            "扩展报的 sent 记进 model.called：{each:?}"
        );
    }
    assert_eq!(call.usage.as_ref().map(|usage| usage.uncached), Some(7));
    let replied: Vec<String> = home
        .log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::MessageAssistant(reply) => Some(reply.blocks),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text),
            _ => None,
        })
        .collect();
    assert_eq!(replied, ["hi from ag"]);
    // 主请求以外还有起标题这类旁路请求，也走这一家：每一次都只交这一家的 key。
    let params = fake.params();
    assert!(!params.is_empty());
    for one in &params {
        assert_eq!(one["key"], "sk-ag");
        assert!(one.get("base_url").is_none(), "没写地址的不交");
        assert!(!one.to_string().contains("sk-b-secret"), "别家的 key 不交");
    }
    assert!(unused.received().is_empty());
}

#[tokio::test]
async fn the_declared_key_rule_decides_what_is_handed_over() {
    let unused = Server::start(hellos(1)).await;
    // `none`：配置里写了也不交。
    let fake = Fake::new(declared(DriverKey::None));
    let (home, handle) = talk(&fake, &two(true, "ag/m", &unused.base_url)).await;
    assert!(called(&home, &handle)[0].error.is_none());
    assert!(fake.params()[0].get("key").is_none(), "不要 key 的不交");
    // `required`：没写 key 的当场 `no_model`，不发。
    let fake = Fake::new(declared(DriverKey::Required));
    let (home, handle) = talk(&fake, &two(false, "ag/m", &unused.base_url)).await;
    let calls = called(&home, &handle);
    let error = calls[0].error.as_ref().expect("出错");
    assert_eq!(
        (error.class.clone(), error.message.as_str()),
        (ErrorClass::NoModel, r#"provider "ag" has no usable key"#)
    );
    assert!(fake.params().is_empty(), "没发");
}

#[tokio::test]
async fn an_absent_driver_is_no_model_and_a_pool_moves_on() {
    let b = Server::start(hellos(1)).await;
    let fake = Fake::new(Err("the package is turned off".to_string()));
    let (home, handle) = talk(&fake, &two(true, "ag/m", &b.base_url)).await;
    let calls = called(&home, &handle);
    let error = calls[0].error.as_ref().expect("出错");
    assert_eq!(
        (error.class.clone(), error.message.as_str()),
        (
            ErrorClass::NoModel,
            r#"model driver "pi/ag" is unavailable: the package is turned off"#
        )
    );
    let (home, handle) = talk(&fake, &two(true, "@p", &b.base_url)).await;
    let calls = called(&home, &handle);
    assert!(calls.last().is_some_and(|call| call.error.is_none()));
    assert_eq!(b.received().len(), 1, "池里换到了下一个成员");
    assert!(fake.params().is_empty(), "不在的驱动一次都没调");
}
