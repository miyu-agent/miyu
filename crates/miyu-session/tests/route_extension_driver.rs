//! 供应商写了扩展提供的驱动（施工 MD-1，设计 66 第四节第 5 条、第八节第 5 条，core1 11-07 定）：MD-2 接上通道以前一律当用不了，
//! 绝不拿主人给扩展配的 key、地址去走内置的驱动：单个模型当场 `no_model`、一个请求都不发；池里的照现成的规矩跳过、换下一个。
//! 写了没装的（配置记警告 `unknown_driver`、值照留）也一样。

use std::time::Duration;

use crate::support::Home;
use crate::support::routing::{called, configs_with_unknown_drivers, hellos, routes, turn};
use miyu_http::testkit::Server;
use miyu_kernel::event::ErrorClass;

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
        home.configs = configs_with_unknown_drivers(&format!(
            "{}[models]\nchat = \"ag/m\"\n",
            extension(driver, &server.base_url)
        ));
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
    home.configs = configs_with_unknown_drivers(&format!(
        "{}[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"@p\"\n\n[pools.p]\nmodels = [\"ag/m\", \"b/m\"]\n",
        extension("gone/drv", &ag.base_url),
        other.base_url
    ));
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
