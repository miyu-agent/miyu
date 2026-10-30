//! 记下的答在 V-25 下加的两件事：说不出来的立一下（`Recalling::missed`）、换了目录照新目录重算（`rebuilt`）。从 `tests.rs`
//! 拆出来，用它的输入和比法。

use super::*;

/// 说不出来的立一下（施工 V-25 下，`Recalling::missed`）：供应商那一面、模型那一面都立；记过的、借得到目录的不立。
#[test]
fn an_unknown_answer_raises_the_missed_flag() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let loaded = loaded();
    let vendors = vendors();
    let recalled = Mutex::new(Recalled::new("trimmed 1", loaded.source, &loaded.fetched));
    let flag = AtomicBool::new(false);
    let lookup = |catalog| {
        CatalogLookup::Recalled(Recalling {
            recalled: &recalled,
            catalog,
            remember: true,
            missed: Some(&flag),
        })
    };
    // 借得到目录：说得出来，不立；顺手记下。
    assert!(
        lookup(Some(&loaded))
            .provider_side("deepseek", None, None)
            .is_some()
    );
    lookup(Some(&loaded)).model_side(&vendors, Some("deepseek"), "deepseek-flash", None);
    assert!(!flag.load(Ordering::Relaxed));
    // 记过的：不借目录也说得出来，不立。
    assert!(lookup(None).provider_side("deepseek", None, None).is_some());
    lookup(None).model_side(&vendors, Some("deepseek"), "deepseek-flash", None);
    assert!(!flag.load(Ordering::Relaxed));
    // 没记过、又没有目录：当认不出，立一下。
    assert_eq!(lookup(None).provider_side("anthropic", None, None), None);
    assert!(
        flag.swap(false, Ordering::Relaxed),
        "供应商那一面说不出来要立"
    );
    assert_eq!(
        lookup(None).model_side(&vendors, None, "claude-sonnet-4-5", None),
        ModelSide::nothing()
    );
    assert!(flag.load(Ordering::Relaxed), "模型那一面说不出来要立");
}

/// 裁出来的目录改几处当成新拉的一份：DeepSeek 的一个模型窗口变了、价格变了，中转的一家整家没了，原厂加了一个模型。
fn changed() -> Loaded {
    let mut json: serde_json::Value =
        serde_json::from_str(crate::test_support::TRIMMED).expect("是 JSON");
    let flash = &mut json["deepseek"]["models"]["deepseek-flash"];
    assert!(flash.is_object(), "裁出来的目录里有 deepseek-flash");
    flash["limit"]["context"] = serde_json::json!(4096);
    flash["cost"]["input"] = serde_json::json!(9.5);
    let removed = json
        .as_object_mut()
        .and_then(|providers| providers.remove("aihubmix"));
    assert!(removed.is_some(), "裁出来的目录里有 aihubmix");
    json["anthropic"]["models"]["claude-newest"] = serde_json::json!({
        "id": "claude-newest", "name": "Claude Newest", "family": "claude",
        "limit": {"context": 300000, "output": 64000}
    });
    Loaded {
        catalog: Catalog::parse(&json.to_string()).expect("读得进").catalog,
        source: CatalogSource::Cache,
        fetched: "2026-10-30T00:00:00.000Z".to_string(),
    }
}

/// 换了目录（施工 V-25 下）：照新目录把记下的每个输入重算，答的和新目录现算逐字一样；版本串、哪一份、拉的时刻跟着换；旧的
/// 一份不动。
#[test]
fn a_rebuilt_recall_answers_like_the_new_catalog() {
    let old = loaded();
    let new = changed();
    let vendors = vendors();
    let providers = provider_asks(&old.catalog);
    let models = model_asks(&old.catalog);
    let recalled = Mutex::new(Recalled::new("old", old.source, &old.fetched));
    let filling = CatalogLookup::Recalled(Recalling {
        recalled: &recalled,
        catalog: Some(&old),
        remember: true,
        missed: None,
    });
    for (id, url, written) in &providers {
        filling.provider_side(id, url.as_deref(), written.as_deref());
    }
    for (recognized, model, written) in &models {
        filling.model_side(&vendors, recognized.as_deref(), model, written.as_deref());
    }
    let before = locked(&recalled).clone();
    let rebuilt = Mutex::new(before.rebuilt(&new, &vendors, "new"));
    assert_eq!(*locked(&recalled), before, "旧的一份不动");
    {
        let held = locked(&rebuilt);
        assert_eq!(held.len(), before.len(), "记下的键一条不少");
        assert_eq!(
            (held.of(), held.source(), held.fetched()),
            ("new", CatalogSource::Cache, new.fetched.as_str())
        );
    }
    let full = CatalogLookup::Full(&new);
    let expected_providers: Vec<_> = providers
        .iter()
        .map(|(id, url, written)| full.provider_side(id, url.as_deref(), written.as_deref()))
        .collect();
    let expected_models: Vec<_> = models
        .iter()
        .map(|(recognized, model, written)| {
            full.model_side(&vendors, recognized.as_deref(), model, written.as_deref())
        })
        .collect();
    // 新旧两份目录答的真有不一样的，重算才算测到了。
    let old_full = CatalogLookup::Full(&old);
    let differs = models
        .iter()
        .zip(&expected_models)
        .any(|((recognized, model, written), new)| {
            old_full.model_side(&vendors, recognized.as_deref(), model, written.as_deref()) != *new
        });
    assert!(differs, "新目录要改到记下的答");
    let flag = std::sync::atomic::AtomicBool::new(false);
    ask_all(
        CatalogLookup::Recalled(Recalling {
            recalled: &rebuilt,
            catalog: None,
            remember: true,
            missed: Some(&flag),
        }),
        &providers,
        &expected_providers,
        &models,
        &expected_models,
        "重算过的",
    );
    assert!(
        !flag.load(std::sync::atomic::Ordering::Relaxed),
        "记下的键全都重算了，不借目录也答得出来"
    );
    assert_eq!(full.fetched(), new.fetched);
}
