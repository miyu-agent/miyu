//! `model.list` 不读整份目录（施工 V-32，`docs/blueprint/models.md`「怎么走」第二条第 2 条第 7 款）：裁出来的目录里每一家都
//! 配上（加照地址认的、认不出的、手写 `catalog` 的、列表里才有的模型），照整份目录现算（常驻）和照记下的（`listing.json`）
//! 两种，`model.list` 列整份、只看一家、只要几个、带 `refresh` 的回应逐字比；头一回读一次整份目录、记下，以后一次都不读；
//! 记下的版本不对、坏了的当没有、重记；配置里新加的一家读一次补上；后台拉到新目录的照新目录重算、不读；超过上限的那一家
//! 不记、每次照旧读，答的照样一样。读整份目录的办法是注入的，数着读了几次。

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_kernel::time::Timestamp;
use miyu_models::catalog::{Catalog, CatalogSource, Loaded, Recalled};
use miyu_models::matching::Vendors;
use miyu_models::observed::{ListedModel, ProviderList};
use miyu_models::profile::Profiles;
use miyu_models::settings::{
    CatalogSettings, ModelSettings, PoolSettings, PriceSettings, ProviderSettings, UseSettings,
};
use miyu_session::{CatalogReader, LISTING_CAP, LISTING_FILE, ModelData, Models, Observed, Routes};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog as ToolCatalog;

use crate::support::*;

const TRIMMED: &str = include_str!("../../miyu-models/testdata/models-dev-trimmed.json");

/// 裁出来的目录里每一家，加照地址认的、认不出的两家，手写了资料的、手写 `catalog` 的、列表里才有的模型，用途、池。
const CONFIG: &str = "[ui]\nlanguage = \"en\"\n\n\
[providers.deepseek]\nkey = { env = \"DEEPSEEK_API_KEY\" }\n\n\
[providers.deepseek.models.\"deepseek-flash\"]\neffort = \"high\"\n\n\
[providers.opencode-go]\nlocal = false\n\n\
[providers.opencode]\nlocal = false\n\n\
[providers.above]\nlocal = false\n\n\
[providers.aihubmix]\nlocal = false\n\n\
[providers.anthropic]\nkey = { env = \"ANTHROPIC_API_KEY\" }\n\n\
[providers.openai]\nlocal = false\n\n\
[providers.302ai]\nlocal = false\n\n\
[providers.deepinfra]\nlocal = false\n\n\
[providers.embedco]\nlocal = false\n\n\
[providers.relay]\nbase_url = \"https://api.deepseek.com/v1\"\n\n\
[providers.relay.models.x]\ncatalog = \"anthropic/claude-sonnet-4-5\"\n\n\
[providers.custom]\ndriver = \"openai-chat\"\nbase_url = \"https://relay.example.invalid/v1\"\nlocal = false\n\n\
[models]\nchat = \"deepseek/deepseek-flash\"\n\n\
[pools.p]\nmodels = [\"custom/custom-7b\", \"relay/deepseek-flash\"]\n";

/// 配置里新加的两家：目录里那一家还没列过的（第一次列它要补），和照手写 `catalog` 认成列过的那一家的。
const ADDED: &str = "\n[providers.llmgateway]\nlocal = false\n\n\
[providers.zen]\ncatalog = \"opencode\"\nbase_url = \"https://zen.example.invalid/v1\"\n";

/// 模型数过上限的一家。
const BIG: &str = "\n[providers.bigco]\nlocal = false\n";

const ENV: &[(&str, &str)] = &[("DEEPSEEK_API_KEY", "sk-1")];

/// 裁出来的目录，加一家能出向量的（`family` 认得出的、名字认得出的）、一家模型数过上限的；`changed` 的当成后来拉的一份：
/// DeepSeek 的一个模型窗口变了、多了一个模型，`above` 整家没了。
fn catalog(changed: bool) -> Loaded {
    let mut json: Value = serde_json::from_str(TRIMMED).expect("是 JSON");
    json["embedco"] = json!({
        "id": "embedco", "name": "Embed Co", "npm": "@ai-sdk/openai-compatible",
        "api": "https://embed.example.invalid/v1",
        "models": {
            "e5-large": {"id": "e5-large", "name": "E5", "family": "bge"},
            "text-embedding-3": {"id": "text-embedding-3", "name": "TE3"},
            "chat-1": {"id": "chat-1", "name": "Chat", "family": "chat", "limit": {"context": 8000, "output": 1000}}
        }
    });
    let models: serde_json::Map<String, Value> = (0..=LISTING_CAP)
        .map(|n| {
            let id = format!("m{n:04}");
            (id.clone(), json!({"id": id, "family": "big"}))
        })
        .collect();
    json["bigco"] = json!({
        "id": "bigco", "name": "Big Co", "npm": "@ai-sdk/openai-compatible",
        "api": "https://big.example.invalid/v1", "models": models
    });
    if changed {
        json["deepseek"]["models"]["deepseek-flash"]["limit"]["context"] = json!(4096);
        json["deepseek"]["models"]["deepseek-newest"] =
            json!({"id": "deepseek-newest", "name": "Newest"});
        json.as_object_mut()
            .map(|providers| providers.remove("above"));
    }
    Loaded {
        catalog: Catalog::parse(&json.to_string()).expect("读得进").catalog,
        source: if changed {
            CatalogSource::Cache
        } else {
            CatalogSource::Snapshot
        },
        fetched: if changed {
            "2026-10-31T00:00:00.000Z"
        } else {
            "2026-10-01T03:25:54.000Z"
        }
        .to_string(),
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

/// 供应商的列表：deepseek 列出一个目录里没有的。
fn observed() -> Observed {
    let mut observed = Observed::default();
    observed.lists.insert(
        "deepseek".to_string(),
        ProviderList {
            fetched: Timestamp::from_unix_millis(1_761_800_000_000).expect("是个时刻"),
            models: ["deepseek-flash", "deepseek-secret"]
                .map(|id| ListedModel {
                    id: id.to_string(),
                    window: Some(64_000),
                    ..ListedModel::default()
                })
                .to_vec(),
        },
    );
    observed
}

fn config(home: &Home, text: &str) -> Config {
    home.write("etc/config.toml", text);
    Config::load(&home.root, None, items(), Environment::of(ENV))
}

/// 常驻的整份目录。
fn resident(changed: bool) -> Arc<ModelData> {
    let data = ModelData::new(profiles(), vendors(), None);
    data.loaded(Some(catalog(changed)), observed());
    Arc::new(data)
}

/// 不常驻：起来时照配置记好了路由那份（版本串 `of`），写盘在 `state`；读整份目录数在 `count` 里。
fn recalling(
    state: &Path,
    of: &'static str,
    text: &str,
    count: &Arc<AtomicUsize>,
) -> Arc<ModelData> {
    let home = Home::new();
    let config = config(&home, text);
    let data = ModelData::new(
        profiles(),
        vendors(),
        Some(miyu_session::ModelDirs::in_one(state.to_path_buf())),
    );
    let counting = Arc::clone(count);
    let reader: CatalogReader = Arc::new(move || {
        counting.fetch_add(1, Ordering::SeqCst);
        Some((catalog(false), of.to_string()))
    });
    let loaded = catalog(false);
    let filled = data.fill(
        Recalled::new(of, loaded.source, &loaded.fetched),
        &loaded,
        config.resolved(),
    );
    data.recalled(filled, observed(), reader);
    Arc::new(data)
}

/// 协议上问 `model.list`：列整份、只看每一家、只要几个（目录里的、列表里才有的、配置里写的、没有的）、带 `refresh` 的。
async fn listed(data: &Arc<ModelData>, text: &str, providers: &[&str]) -> String {
    let mut asks = vec![
        json!({}),
        json!({"refresh": true}),
        json!({"models": ["deepseek/deepseek-secret", "deepseek/deepseek-v4-flash", "relay/x", "embedco/e5-large", "custom/custom-7b", "nope/x", "deepseek/nope"]}),
        json!({"provider": "deepseek", "models": ["deepseek/deepseek-flash"], "refresh": true}),
    ];
    asks.extend(providers.iter().map(|id| json!({"provider": id})));
    ask(data, text, asks).await
}

/// 照配置 `text` 起一个核心（模型资料是 `data`），一个头连上来问 `asks` 里的每一样 `model.list`，回应写成一排。
async fn ask(data: &Arc<ModelData>, text: &str, asks: Vec<Value>) -> String {
    let home = Home::new();
    let config = config(&home, text);
    let models: Arc<dyn Models> = Arc::new(Routes {
        client: miyu_http::client(miyu_http::Proxy::Off).expect("造得出客户端"),
        direct: miyu_http::client(miyu_http::Proxy::Off).expect("造得出客户端"),
        data: Arc::clone(data),
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
    let core = Arc::new(core.with_config(config).with_model_data(Arc::clone(data)));
    let mut client = Client::connect(core);
    client.hello().await;
    let mut said = Vec::new();
    for params in asks {
        said.push(client.call("l", "model.list", params).await);
    }
    Value::Array(said).to_string()
}

fn ids(text: &str) -> Vec<&'static str> {
    let mut ids = vec![
        "deepseek",
        "opencode-go",
        "opencode",
        "above",
        "aihubmix",
        "anthropic",
        "openai",
        "302ai",
        "deepinfra",
        "embedco",
        "relay",
        "custom",
    ];
    if text.contains("[providers.zen]") {
        ids.extend(["llmgateway", "zen"]);
    }
    if text.contains("[providers.bigco]") {
        ids.push("bigco");
    }
    ids
}

/// 读回 `listing.json` 的版本串；没有的、读不懂的没有。
fn listing_of(state: &Path) -> Option<String> {
    let text = std::fs::read_to_string(state.join(LISTING_FILE)).ok()?;
    Recalled::from_json(&text)
        .ok()
        .map(|recalled| recalled.of().to_string())
}

#[tokio::test]
async fn listing_from_recalled_answers_byte_for_byte_like_the_full_catalog() {
    let scratch = Home::new();
    let state = scratch.root.path().join("state-models");
    std::fs::create_dir_all(&state).expect("建得了");
    let count = Arc::new(AtomicUsize::new(0));
    let data = recalling(&state, "v1", CONFIG, &count);
    let expected = listed(&resident(false), CONFIG, &ids(CONFIG)).await;
    for needle in [
        "\"embedding\":true",
        "\"how\":\"url\"",
        "deepseek-secret",
        "\"value\":\"Embed Co\"",
        "\"problem\"",
        "\"listed\":[\"config\",\"provider\",\"catalog\"]",
        "\"listed\":[\"provider\"]",
        "\"listed\":[\"config\"]",
    ] {
        assert!(
            expected.contains(needle),
            "常驻的回应里没有 {needle}：{expected}"
        );
    }
    assert_eq!(
        count.load(Ordering::SeqCst),
        0,
        "起来时记好了路由那份，不读"
    );

    // 头一回：读一次整份目录、记下；第二回一次都不读。
    assert_eq!(
        listed(&data, CONFIG, &ids(CONFIG)).await,
        expected,
        "头一回"
    );
    let first = count.load(Ordering::SeqCst);
    assert_eq!(first, 1, "头一回只读一次");
    assert_eq!(listing_of(&state).as_deref(), Some("v1"));
    assert_eq!(
        listed(&data, CONFIG, &ids(CONFIG)).await,
        expected,
        "照记下的"
    );
    assert_eq!(count.load(Ordering::SeqCst), first, "记好了不读整份目录");

    // 核心重启：照盘上的两份答，一次都不读。
    let again = Arc::new(AtomicUsize::new(0));
    let restarted = recalling(&state, "v1", CONFIG, &again);
    assert_eq!(
        listed(&restarted, CONFIG, &ids(CONFIG)).await,
        expected,
        "重启以后"
    );
    assert_eq!(again.load(Ordering::SeqCst), 0, "重启以后照记下的");

    // 坏了的当没有：读一次、重记。
    std::fs::write(state.join(LISTING_FILE), "{not json").expect("写得了");
    let broken = Arc::new(AtomicUsize::new(0));
    let data = recalling(&state, "v1", CONFIG, &broken);
    assert_eq!(
        listed(&data, CONFIG, &ids(CONFIG)).await,
        expected,
        "坏了的"
    );
    assert_eq!(broken.load(Ordering::SeqCst), 1, "坏了的读一次");
    assert_eq!(listing_of(&state).as_deref(), Some("v1"), "重记了");

    // 版本对不上（目录换了、程序换了）的当没有：读一次、照新的版本串重记。
    let other = Arc::new(AtomicUsize::new(0));
    let data = recalling(&state, "v2", CONFIG, &other);
    assert_eq!(
        listed(&data, CONFIG, &ids(CONFIG)).await,
        expected,
        "版本换了"
    );
    assert_eq!(other.load(Ordering::SeqCst), 1, "版本换了读一次");
    assert_eq!(listing_of(&state).as_deref(), Some("v2"));

    // 配置里新加的一家：读一次补上，原来的照旧。
    let added = format!("{CONFIG}{ADDED}");
    let want = listed(&resident(false), &added, &ids(&added)).await;
    assert!(want.contains("\"id\":\"zen\"") && want.contains("\"how\":\"config\""));
    // 路由那份先照新配置补（核心的 `follow_recall`，在阻塞线程里），读一次；列整份时目录里那一家没列过，再读一次。
    let before = other.load(Ordering::SeqCst);
    let home = Home::new();
    data.remember_configured(config(&home, &added).resolved());
    assert_eq!(other.load(Ordering::SeqCst), before + 1, "路由那份补一次");
    assert_eq!(
        listed(&data, &added, &ids(&added)).await,
        want,
        "新加的一家"
    );
    assert_eq!(other.load(Ordering::SeqCst), before + 2, "列整份补一次");
    assert_eq!(listed(&data, &added, &ids(&added)).await, want);
    assert_eq!(other.load(Ordering::SeqCst), before + 2, "补上了不再读");

    // 后台拉到新目录：照新目录重算记下的两份，不读，答的和新目录常驻的一样。
    data.replace_catalog(catalog(true), Some("v3".to_string()));
    assert_eq!(listing_of(&state).as_deref(), Some("v3"), "跟着重算");
    let want = listed(&resident(true), &added, &ids(&added)).await;
    assert_ne!(
        want,
        listed(&resident(false), &added, &ids(&added)).await,
        "新旧两份要真不一样"
    );
    let before = other.load(Ordering::SeqCst);
    assert_eq!(listed(&data, &added, &ids(&added)).await, want, "新目录");
    assert_eq!(other.load(Ordering::SeqCst), before, "重算过的不读");
}

#[tokio::test]
async fn a_provider_over_the_cap_is_not_remembered_but_answers_the_same() {
    let scratch = Home::new();
    let state = scratch.root.path().join("state-models");
    std::fs::create_dir_all(&state).expect("建得了");
    let text = format!("{CONFIG}{BIG}");
    let count = Arc::new(AtomicUsize::new(0));
    let data = recalling(&state, "v1", &text, &count);
    let expected = listed(&resident(false), &text, &ids(&text)).await;
    assert!(expected.contains("m1000"));
    for round in 1..=2 {
        let before = count.load(Ordering::SeqCst);
        assert_eq!(
            listed(&data, &text, &ids(&text)).await,
            expected,
            "第 {round} 回"
        );
        assert!(
            count.load(Ordering::SeqCst) > before,
            "过了上限的那一家每次照旧读"
        );
    }
    let stored = std::fs::read_to_string(state.join(LISTING_FILE)).expect("别的几家记下了");
    assert!(!stored.contains("m1000"), "过了上限的那一家不记");
    assert!(stored.contains("deepseek-secret"), "别的几家照记");
    // 不问过了上限的那一家的，一次都不读。
    let before = count.load(Ordering::SeqCst);
    let small: Vec<Value> = ids(CONFIG)
        .iter()
        .map(|id| json!({"provider": id}))
        .collect();
    assert_eq!(
        ask(&data, &text, small.clone()).await,
        ask(&resident(false), &text, small).await
    );
    assert_eq!(
        count.load(Ordering::SeqCst),
        before,
        "只问没过上限的几家不读"
    );
}
