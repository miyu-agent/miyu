//! 目录不常驻（施工 V-25 下，`docs/blueprint/models.md`「怎么走」第二条第 2 条）：配置里写过的照整份目录记好以后，路由问
//! 资料一次都不读整份目录；没记过的输入读一次、记下、写盘，并发的两个只读一次；不记的问一次读一次；读回来的版本和记下的
//! 不一样的、后台拉到新目录的，照新目录把记下的键全部重算，答的和新目录常驻时逐字一样。读整份目录的办法是注入的，数着
//! 读了几次。

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Duration;

use serde_json::json;

use crate::support::Scratch;
use crate::support::routing::resolved;
use miyu_config::merge::Resolved;
use miyu_models::catalog::{Catalog, CatalogSource, Loaded, Recalled};
use miyu_models::facts::facts;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::provider::provider;
use miyu_session::{CatalogReader, ModelData, Observed, RECALL_FILE};

const TRIMMED: &str = include_str!("../../miyu-models/testdata/models-dev-trimmed.json");

/// 真目录裁出来的一份，当作 10-01 拉的快照。
fn trimmed() -> Loaded {
    Loaded {
        catalog: Catalog::parse(TRIMMED).expect("读得进").catalog,
        source: CatalogSource::Snapshot,
        fetched: "2026-10-01T03:25:54.000Z".to_string(),
    }
}

/// 同一份改几处当成后来拉的：DeepSeek 的 `deepseek-flash` 窗口、价格变了，`aihubmix` 整家没了。
fn changed() -> Loaded {
    let mut json: serde_json::Value = serde_json::from_str(TRIMMED).expect("是 JSON");
    json["deepseek"]["models"]["deepseek-flash"]["limit"]["context"] = json!(4096);
    json["deepseek"]["models"]["deepseek-flash"]["cost"]["input"] = json!(9.5);
    if let Some(providers) = json.as_object_mut() {
        providers.remove("aihubmix");
    }
    Loaded {
        catalog: Catalog::parse(&json.to_string()).expect("读得进").catalog,
        source: CatalogSource::Cache,
        fetched: "2026-10-30T00:00:00.000Z".to_string(),
    }
}

/// 配了几家、几个模型、一个池：照编号认的、去掉分隔认的、照地址认的、认不出的，手写指定不存在的。
const CONFIG: &str = "[providers.deepseek]\nlocal = false\n\n\
[providers.deepseek.models.\"deepseek-v4-pro\"]\nwindow = 50000\n\n\
[providers.opencodego]\nlocal = false\n\n\
[providers.newapi]\ndriver = \"openai-chat\"\nbase_url = \"https://relay.example.invalid/v1\"\nlocal = false\n\n\
[providers.newapi.models.x]\ncatalog = \"deepseek/nope\"\n\n\
[providers.proxy]\nbase_url = \"https://api.deepseek.com/v1\"\n\n\
[models]\nchat = \"deepseek/deepseek-flash\"\nvision = \"newapi/claude-sonnet-4-5\"\n\n\
[pools.p]\nmodels = [\"opencodego/deepseek-v4.1-flash\", \"proxy/deepseek-flash\", \"newapi/custom-7b\"]\n";

/// 配置里写过的：供应商、模型。
fn configured() -> (Vec<&'static str>, Vec<(&'static str, &'static str)>) {
    (
        vec!["deepseek", "opencodego", "newapi", "proxy"],
        vec![
            ("deepseek", "deepseek-v4-pro"),
            ("deepseek", "deepseek-flash"),
            ("newapi", "x"),
            ("newapi", "claude-sonnet-4-5"),
            ("opencodego", "deepseek-v4.1-flash"),
            ("proxy", "deepseek-flash"),
            ("newapi", "custom-7b"),
        ],
    )
}

fn profiles() -> Profiles {
    Profiles::parse(&json!({"npm": {
        "@ai-sdk/openai-compatible": "openai-chat",
        "@ai-sdk/anthropic": "anthropic",
        "@ai-sdk/openai": "openai-responses"
    }}))
    .expect("档案写法对")
}

fn vendors() -> Vendors {
    Vendors::parse(&json!({"claude": ["anthropic"], "gpt": ["openai"], "deepseek": ["deepseek"]}))
        .expect("读得进")
}

/// 常驻的一份：读完了 `loaded`。
fn resident(loaded: Loaded) -> ModelData {
    let data = ModelData::new(profiles(), vendors(), None);
    data.loaded(Some(loaded), Observed::default());
    data
}

/// 读整份目录的办法：每读一次数一下、先睡 `delay`，交回 `make()` 造的目录和版本串 `of`。
fn reader(
    count: &Arc<AtomicUsize>,
    of: &'static str,
    delay: Duration,
    make: fn() -> Loaded,
) -> CatalogReader {
    let count = Arc::clone(count);
    Arc::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(delay);
        Some((make(), of.to_string()))
    })
}

/// 不常驻的一份：记下的是 `recalled`，写盘在 `dir`。
fn recalling(recalled: Recalled, dir: Option<PathBuf>, reader: CatalogReader) -> ModelData {
    let data = ModelData::new(profiles(), vendors(), dir);
    data.recalled(recalled, Observed::default(), reader);
    data
}

/// 照配置写过的问一遍：每一家合出来的供应商（连同出错），每个模型的资料（`Debug` 和 `model.list` 的写法）。
fn answers(data: &ModelData, resolved: &Resolved) -> Vec<String> {
    let values = resolved.values();
    let (providers, models) = configured();
    let mut said = Vec::new();
    for id in providers {
        said.push(data.with(|knowledge| format!("{:?}", provider(&values, knowledge, id))));
    }
    for (id, model) in models {
        said.push(data.with(|knowledge| {
            let Ok(found) = provider(&values, knowledge, id) else {
                return format!("{id} 用不了");
            };
            let (facts, matched) = facts(resolved, knowledge, &found, model);
            format!(
                "{facts:?} {matched:?} {}",
                facts.json(&|_| "config.toml".to_string())
            )
        }));
    }
    said
}

/// 记好了配置里写过的：路由每一轮问的一次都不读整份目录，答的和常驻时逐字一样。
#[test]
fn what_the_config_wrote_is_answered_without_reading_the_catalog() {
    let resolved = resolved(CONFIG);
    let resident = resident(trimmed());
    let filled = resident.fill(
        Recalled::new("v1", CatalogSource::Snapshot, &trimmed().fetched),
        &trimmed(),
        &resolved,
    );
    let count = Arc::new(AtomicUsize::new(0));
    let data = recalling(filled, None, reader(&count, "v1", Duration::ZERO, trimmed));
    assert!(data.is_loaded(), "记下的那份换上了就算读完");
    assert_eq!(answers(&resident, &resolved), answers(&data, &resolved));
    assert_eq!(count.load(Ordering::SeqCst), 0, "一次都没读整份目录");
    // 要整份目录的才临时读。
    let holding = data.hold();
    assert!(holding.catalog().is_some());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        holding.meta(),
        Some((CatalogSource::Snapshot, trimmed().fetched))
    );
}

/// 没记过的输入：读一次整份目录、记下、写盘，以后照记下的；不记的问一次读一次，不落盘。
#[test]
fn an_input_not_recalled_is_read_once_remembered_and_saved() {
    let scratch = Scratch::new();
    let dir = scratch.0.join("models");
    std::fs::create_dir_all(&dir).expect("建得了");
    let resolved = resolved(CONFIG);
    let resident = resident(trimmed());
    let count = Arc::new(AtomicUsize::new(0));
    let data = recalling(
        Recalled::new("v1", CatalogSource::Snapshot, &trimmed().fetched),
        Some(dir.clone()),
        reader(&count, "v1", Duration::ZERO, trimmed),
    );
    let empty =
        Recalled::from_json(&std::fs::read_to_string(dir.join(RECALL_FILE)).expect("换上时写了"))
            .expect("读得回来");
    assert_eq!(empty.len(), (0, 0));
    let first = answers(&data, &resolved);
    let reads = count.load(Ordering::SeqCst);
    assert!(reads >= 1, "没记过的要读");
    assert_eq!(first, answers(&resident, &resolved), "答的和常驻时一样");
    assert_eq!(answers(&data, &resolved), first);
    assert_eq!(count.load(Ordering::SeqCst), reads, "记下了，再问不读");
    let saved = Recalled::from_json(&std::fs::read_to_string(dir.join(RECALL_FILE)).expect("写了"))
        .expect("读得回来");
    let (providers, models) = saved.len();
    assert!(
        providers >= 4 && models >= 1,
        "记下的写了盘：{:?}",
        saved.len()
    );

    // 临时问一句的（`provider.test`）不记：问一次读一次。
    let values = resolved.values();
    let once = || {
        data.with_once(|knowledge| {
            // 作废的那一遍看到的是没有目录时的答（用不了），别在里面断言。
            let found = provider(&values, knowledge, "deepseek").ok()?;
            facts(&resolved, knowledge, &found, "deepseek-v4-flash")
                .0
                .window
                .value
        })
    };
    assert_eq!(once(), Some(1_000_000));
    assert_eq!(once(), Some(1_000_000));
    assert_eq!(count.load(Ordering::SeqCst), reads + 2, "不记的每次都读");
    let after = Recalled::from_json(&std::fs::read_to_string(dir.join(RECALL_FILE)).expect("写了"))
        .expect("读得回来");
    assert_eq!(after.len(), saved.len(), "不记的不落盘");
}

/// 两个线程同时遇到同一个没记过的输入：整份目录只读一次，后来的照前一个记下的答。
#[test]
fn two_at_once_missing_the_same_input_read_the_catalog_once() {
    let resolved = resolved(CONFIG);
    let count = Arc::new(AtomicUsize::new(0));
    let data = Arc::new(recalling(
        Recalled::new("v1", CatalogSource::Snapshot, &trimmed().fetched),
        None,
        reader(&count, "v1", Duration::from_millis(300), trimmed),
    ));
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let (data, barrier, resolved) =
                (Arc::clone(&data), Arc::clone(&barrier), resolved.clone());
            std::thread::spawn(move || {
                barrier.wait();
                let values = resolved.values();
                data.with(|knowledge| {
                    // 作废的那一遍看到的是没有目录时的答（用不了），别在里面断言。
                    let found = provider(&values, knowledge, "deepseek").ok()?;
                    facts(&resolved, knowledge, &found, "deepseek-flash")
                        .0
                        .window
                        .value
                })
            })
        })
        .collect();
    for thread in threads {
        assert_eq!(thread.join().expect("没有 panic"), Some(1_000_000));
    }
    assert_eq!(count.load(Ordering::SeqCst), 1, "并发的两次只读一次");
}

/// 临时读回来的版本和记下的不一样（缓存被别的核心换过）：照它把记下的全部重算、换上、写盘；答的和新目录常驻时一样。
#[test]
fn a_different_version_read_back_rebuilds_the_recall() {
    let scratch = Scratch::new();
    let dir = scratch.0.join("models");
    std::fs::create_dir_all(&dir).expect("建得了");
    let resolved = resolved(CONFIG);
    let filled = resident(trimmed()).fill(
        Recalled::new("v1", CatalogSource::Snapshot, &trimmed().fetched),
        &trimmed(),
        &resolved,
    );
    let count = Arc::new(AtomicUsize::new(0));
    let data = recalling(
        filled,
        Some(dir.clone()),
        reader(&count, "v2", Duration::ZERO, changed),
    );
    let holding = data.hold();
    assert_eq!(
        holding.meta(),
        Some((CatalogSource::Cache, changed().fetched)),
        "换成了新的一份"
    );
    drop(holding);
    let saved = Recalled::from_json(&std::fs::read_to_string(dir.join(RECALL_FILE)).expect("写了"))
        .expect("读得回来");
    assert_eq!(saved.of(), "v2");
    let reads = count.load(Ordering::SeqCst);
    assert_eq!(
        answers(&data, &resolved),
        answers(&resident(changed()), &resolved)
    );
    assert_eq!(
        count.load(Ordering::SeqCst),
        reads,
        "重算过的键都答得出来，不再读"
    );
}

/// 后台拉到新目录：照它把记下的全部重算、原子地换上、写盘，路由问的答和新目录常驻时逐字一样，一次都不读整份目录。
#[test]
fn a_refresh_rebuilds_every_remembered_key() {
    let scratch = Scratch::new();
    let dir = scratch.0.join("models");
    std::fs::create_dir_all(&dir).expect("建得了");
    let resolved = resolved(CONFIG);
    let filled = resident(trimmed()).fill(
        Recalled::new("v1", CatalogSource::Snapshot, &trimmed().fetched),
        &trimmed(),
        &resolved,
    );
    let count = Arc::new(AtomicUsize::new(0));
    let data = recalling(
        filled,
        Some(dir.clone()),
        reader(&count, "v2", Duration::ZERO, changed),
    );
    let before = answers(&data, &resolved);
    assert_eq!(before, answers(&resident(trimmed()), &resolved));
    data.replace_catalog(changed(), Some("v2".to_string()));
    let after = answers(&data, &resolved);
    assert_ne!(before, after, "新目录改到了记下的答");
    assert_eq!(after, answers(&resident(changed()), &resolved));
    assert_eq!(
        count.load(Ordering::SeqCst),
        0,
        "重算用的是拉到的那一份，不另读"
    );
    let saved = Recalled::from_json(&std::fs::read_to_string(dir.join(RECALL_FILE)).expect("写了"))
        .expect("读得回来");
    assert_eq!(
        (saved.of(), saved.source(), saved.fetched()),
        ("v2", CatalogSource::Cache, changed().fetched.as_str())
    );
    // 算不出版本串的：从此常驻这一份。
    data.replace_catalog(trimmed(), None);
    assert_eq!(
        answers(&data, &resolved),
        answers(&resident(trimmed()), &resolved)
    );
}
