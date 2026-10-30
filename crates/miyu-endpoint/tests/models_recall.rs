//! 目录不常驻和常驻对外逐字一样（施工 V-25 下，`docs/blueprint/models.md`「怎么走」第二条第 2 条）：同一份配置、同一份裁出来
//! 的目录，三种模型资料各导一份，逐字节比——常驻的整份目录；不常驻、起来时照配置记好的（路由问配置里写过的一次都不读整份
//! 目录）；不常驻、什么都没记的（每个输入都是头一回，读整份目录补）。导的是：每一家合出来的供应商（连同出错原话）、所有配置
//! 过的模型和池成员的资料（`Debug` 和 `model.list` 的写法）、每个引用解析的结果和思考强度的档位，协议上的 `model.list` 整份
//! 回应（全部、只看一家、只看几个模型）、`config.get` 报的 `unknown_effort`/`unusable_temperature`、`config.check`、
//! `session.configure` 照档位查的 `effort`。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_models::catalog::{Catalog, CatalogSource, Loaded, Recalled};
use miyu_models::effort::levels_of;
use miyu_models::facts::facts;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::provider::provider;
use miyu_models::reference::resolve;
use miyu_models::settings::{
    CatalogSettings, ModelSettings, PoolSettings, PriceSettings, ProviderSettings, UseSettings,
};
use miyu_session::{CatalogReader, ModelData, Models, Observed, Routes};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog as ToolCatalog;

use crate::support::*;

/// 几家走不同认法的、手写了资料的、思考强度写了没有的一档的、温度写了这个模型用不了的，用途、池。
const CONFIG: &str = "[ui]\nlanguage = \"en\"\n\n\
[providers.deepseek]\nkey = { env = \"DEEPSEEK_API_KEY\" }\n\n\
[providers.deepseek.models.\"deepseek-flash\"]\neffort = \"high\"\n\n\
[providers.deepseek.models.\"deepseek-v4-pro\"]\neffort = \"ultra\"\nwindow = 50000\n\n\
[providers.opencodego]\nlocal = false\n\n\
[providers.newapi]\ndriver = \"openai-chat\"\nbase_url = \"https://relay.example.invalid/v1\"\nlocal = false\nprice_multiplier = 0.5\n\n\
[providers.newapi.models.\"claude-sonnet-4-5\"]\nwindow = 100000\n\n\
[providers.newapi.models.x]\ncatalog = \"deepseek/nope\"\n\n\
[providers.newapi.models.\"gpt-5\"]\ntemperature = 0.5\neffort = \"minimal\"\n\n\
[providers.proxy]\nbase_url = \"https://api.deepseek.com/v1\"\n\n\
[providers.anthropic]\nkey = { env = \"ANTHROPIC_API_KEY\" }\n\n\
[providers.broken]\nlocal = false\n\n\
[models]\nchat = \"deepseek/deepseek-flash\"\nvision = \"anthropic/claude-sonnet-4-5\"\n\n\
[pools.p]\nmodels = [\"opencodego/deepseek-v4.1-flash\", \"newapi/custom-7b\", \"proxy/deepseek-v4-flash\"]\nstrategy = \"rotate\"\n";

/// 配置里写过的模型：手写了资料的、用途、池的成员。
const CONFIGURED: &[(&str, &str)] = &[
    ("deepseek", "deepseek-flash"),
    ("deepseek", "deepseek-v4-pro"),
    ("newapi", "claude-sonnet-4-5"),
    ("newapi", "x"),
    ("newapi", "gpt-5"),
    ("anthropic", "claude-sonnet-4-5"),
    ("opencodego", "deepseek-v4.1-flash"),
    ("newapi", "custom-7b"),
    ("proxy", "deepseek-v4-flash"),
];

/// 配置里没写的：会话记着的、`provider.test` 试的那种。不常驻时第一次问到才读整份目录。
const EXTRA: &[(&str, &str)] = &[
    ("deepseek", "deepseek-v4-flash"),
    ("newapi", "DeepSeek V4 Flash"),
    ("newapi", "gpt-5-minimal"),
    ("broken", "anything"),
    ("opencodego", "gpt-5.6-luna"),
];

const PROVIDERS: &[&str] = &[
    "deepseek",
    "opencodego",
    "newapi",
    "proxy",
    "anthropic",
    "broken",
    "nope",
];

const TRIMMED: &str = include_str!("../../miyu-models/testdata/models-dev-trimmed.json");

fn trimmed() -> Loaded {
    Loaded {
        catalog: Catalog::parse(TRIMMED).expect("读得进").catalog,
        source: CatalogSource::Snapshot,
        fetched: "2026-10-01T03:25:54.000Z".to_string(),
    }
}

fn profiles() -> Profiles {
    Profiles::parse(&json!({
        "npm": {
            "@ai-sdk/openai-compatible": "openai-chat",
            "@ai-sdk/anthropic": "anthropic",
            "@ai-sdk/openai": "openai-responses"
        },
        "providers": {"deepseek": {"driver": "openai-chat", "base_url": "https://api.deepseek.com"}}
    }))
    .expect("档案写法对")
}

fn vendors() -> Vendors {
    Vendors::parse(&json!({"claude": ["anthropic"], "gpt": ["openai"], "deepseek": ["deepseek"]}))
        .expect("读得进")
}

fn items() -> Vec<miyu_config::Item> {
    [
        miyu_endpoint::settings::UiSettings::ITEMS,
        UseSettings::ITEMS,
        PoolSettings::ITEMS,
        miyu_models::settings::FIXED_POOL_ITEMS,
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        PriceSettings::ITEMS,
        CatalogSettings::ITEMS,
    ]
    .concat()
}

const ENV: &[(&str, &str)] = &[("DEEPSEEK_API_KEY", "sk-1")];

/// 三种模型资料。
#[derive(Clone, Copy, Debug)]
enum Kind {
    /// 常驻的整份目录。
    Resident,
    /// 不常驻，起来时照配置记好了。
    Filled,
    /// 不常驻，什么都没记。
    Empty,
}

/// 照 `kind` 造一份模型资料，读整份目录的次数数在 `count` 里。
fn data(kind: Kind, config: &Config, count: &Arc<AtomicUsize>) -> Arc<ModelData> {
    let data = ModelData::new(profiles(), vendors(), None);
    let counting = Arc::clone(count);
    let reader: CatalogReader = Arc::new(move || {
        counting.fetch_add(1, Ordering::SeqCst);
        Some((trimmed(), "v1".to_string()))
    });
    let empty = Recalled::new("v1", CatalogSource::Snapshot, &trimmed().fetched);
    match kind {
        Kind::Resident => data.loaded(Some(trimmed()), Observed::default()),
        Kind::Filled => {
            let filled = data.fill(empty, &trimmed(), config.resolved());
            data.recalled(filled, Observed::default(), reader);
        }
        Kind::Empty => data.recalled(empty, Observed::default(), reader),
    }
    Arc::new(data)
}

/// 直接问模型资料：`refs` 里每个模型的资料、每家供应商、每个引用。
fn direct(data: &ModelData, config: &Config, refs: &[(&str, &str)], providers: &[&str]) -> String {
    let resolved = config.resolved();
    let values = resolved.values();
    let mut said = String::new();
    for id in providers {
        let found = data.with(|knowledge| format!("{:?}", provider(&values, knowledge, id)));
        said.push_str(&format!("provider {id}: {found}\n"));
    }
    for (id, model) in refs {
        let got = data.with(|knowledge| {
            let Ok(found) = provider(&values, knowledge, id) else {
                return "unusable".to_string();
            };
            let (facts, matched) = facts(resolved, knowledge, &found, model);
            format!(
                "{facts:?} {matched:?} {}",
                facts.json(&|_| "config.toml".to_string())
            )
        });
        said.push_str(&format!("facts {id}/{model}: {got}\n"));
        let text = format!("{id}/{model}");
        let resolved_ref =
            data.with(|knowledge| format!("{:?}", resolve(&values, knowledge, &text)));
        let levels = data.with(|knowledge| levels_of(resolved, knowledge, &text));
        said.push_str(&format!(
            "resolve {text}: {resolved_ref} levels {levels:?}\n"
        ));
    }
    for text in ["@p", "@nope"] {
        let got = data.with(|knowledge| format!("{:?}", resolve(&values, knowledge, text)));
        let levels = data.with(|knowledge| levels_of(resolved, knowledge, text));
        said.push_str(&format!("resolve {text}: {got} levels {levels:?}\n"));
    }
    said
}

/// 协议上问一遍：握手报的配置问题个数、`config.get` 的问题、`config.check`、`model.list` 三种问法、`session.configure` 的
/// `effort` 几种。会话编号换成 `S`。
async fn protocol(data: Arc<ModelData>) -> String {
    let home = Home::new();
    home.write("config.toml", CONFIG);
    let config = Config::load(&home.root, None, items(), Environment::of(ENV));
    let models: Arc<dyn Models> = Arc::new(Routes {
        client: miyu_http::client(miyu_http::Proxy::Off).expect("造得出客户端"),
        direct: miyu_http::client(miyu_http::Proxy::Off).expect("造得出客户端"),
        data: Arc::clone(&data),
        idle: std::time::Duration::from_secs(60),
    });
    let core = Core::new(
        home.root.clone(),
        ResourceRoot::at(default_resources()),
        models,
        ToolCatalog::default(),
        None,
        TOKEN.to_string(),
    );
    let core = Arc::new(core.with_config(config).with_model_data(data));
    let mut client = Client::connect(core);
    let hello = client.hello().await;
    let mut said = vec![hello["result"]["config_errors"].clone()];
    said.push(client.call("g", "config.get", json!({})).await["result"]["problems"].clone());
    let text = CONFIG.replace("effort = \"ultra\"", "effort = \"max\"");
    said.push(
        client
            .call(
                "c",
                "config.check",
                json!({"layer": "system", "text": text}),
            )
            .await,
    );
    for params in [
        json!({}),
        json!({"provider": "newapi"}),
        json!({"models": ["deepseek/deepseek-flash", "newapi/x", "opencodego/gpt-5.6-luna"]}),
    ] {
        said.push(client.call("l", "model.list", params).await);
    }
    let session = client.create("s", "/tmp").await;
    for (n, params) in [
        json!({"session": session, "effort": "max"}),
        json!({"session": session, "effort": "ultra"}),
        json!({"session": session, "effort": "low", "model": "newapi/gpt-5"}),
        json!({"session": session, "effort": "minimal", "model": "newapi/gpt-5"}),
        json!({"session": session, "effort": "high", "model": "@p"}),
        json!({"session": session, "effort": "high", "model": "deepseek/deepseek-v4-flash"}),
    ]
    .into_iter()
    .enumerate()
    {
        said.push(
            client
                .call(&format!("e{n}"), "session.configure", params)
                .await,
        );
    }
    Value::Array(said).to_string().replace(&session, "S")
}

#[tokio::test]
async fn recalled_and_resident_answer_byte_for_byte_the_same() {
    let home = Home::new();
    home.write("config.toml", CONFIG);
    let config = Config::load(&home.root, None, items(), Environment::of(ENV));
    let mut dumps = Vec::new();
    for kind in [Kind::Resident, Kind::Filled, Kind::Empty] {
        let count = Arc::new(AtomicUsize::new(0));
        let data = data(kind, &config, &count);
        let configured = direct(&data, &config, CONFIGURED, PROVIDERS);
        let read_for_configured = count.load(Ordering::SeqCst);
        let extra = direct(&data, &config, EXTRA, &[]);
        let read_for_extra = count.load(Ordering::SeqCst);
        let protocol = protocol(Arc::clone(&data)).await;
        match kind {
            Kind::Resident => assert_eq!(read_for_extra, 0),
            Kind::Filled => {
                assert_eq!(read_for_configured, 0, "配置里写过的一次都不读整份目录");
                assert!(read_for_extra > 0, "没写过的要读");
            }
            Kind::Empty => assert!(read_for_configured > 0, "什么都没记的要读"),
        }
        if !matches!(kind, Kind::Resident) {
            assert!(
                count.load(Ordering::SeqCst) > read_for_extra,
                "model.list 要临时读整份目录"
            );
        }
        dumps.push((kind, configured, extra, protocol));
    }
    // 导出来的要真有内容：认出来的、认不出的、手写指定不存在的、报出来的配置问题都在里面。
    let (_, configured, _, protocol) = &dumps[0];
    for needle in [
        "SimilarId",
        "Url",
        "Missing",
        "needs base_url",
        "deepseek-flash",
    ] {
        assert!(
            configured.contains(needle),
            "常驻的导出里没有 {needle}：{configured}"
        );
    }
    for needle in [
        "unknown_effort",
        "unusable_temperature",
        "\"source\":\"snapshot\"",
        "bad_params",
    ] {
        assert!(
            protocol.contains(needle),
            "协议的导出里没有 {needle}：{protocol}"
        );
    }
    let (_, resident_configured, resident_extra, resident_protocol) = &dumps[0];
    for (kind, configured, extra, protocol) in &dumps[1..] {
        assert_eq!(resident_configured, configured, "{kind:?}：配置里写过的");
        assert_eq!(resident_extra, extra, "{kind:?}：没写过的");
        assert_eq!(resident_protocol, protocol, "{kind:?}：协议");
    }
}
