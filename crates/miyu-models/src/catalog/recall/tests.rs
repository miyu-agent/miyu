//! 查目录的两种办法答的逐字一样（施工 V-25 上）：整份目录现算（`Full`）、照记下的答（`Recalled`）。先空着边查边记，再只照
//! 记下的查（不借目录：答得出来就说明全记下了），再写成 JSON 读回来查，每一个输入都和现算的比 `Debug` 的原样。再往上一层：
//! 合出来的供应商、模型的资料、思考强度的档位，两种办法下也逐字一样。

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::sync::{Mutex, PoisonError};

use miyu_config::Layer;

use super::*;
use crate::Knowledge;
use crate::catalog::{Catalog, CatalogLookup, ModelSide, ProviderSide};
use crate::effort::levels_of;
use crate::facts::facts;
use crate::matching::normalize;
use crate::provider::provider;
use crate::test_support::{Held, loaded, resolved, vendors};

/// 两样东西 `Debug` 写出来逐字一样。
fn same<T: Debug>(expected: &T, got: &T, what: &dyn Fn() -> String) {
    assert_eq!(format!("{expected:?}"), format!("{got:?}"), "{}", what());
}

fn some(text: &str) -> Option<String> {
    Some(text.to_string())
}

/// 问供应商的那一面的输入：目录里每一家的编号，加大小写变体、去掉 `-`、`_` 的、几个目录里没有的；几个地址（目录里每一家的、
/// 末尾带 `/`、带 `/v1/`、主机名大写的、没有的）；几个手写的 `catalog`（有的、没有的、空的）。
fn provider_asks(catalog: &Catalog) -> Vec<(String, Option<String>, Option<String>)> {
    let mut ids = Vec::new();
    for listed in catalog.providers() {
        ids.push(listed.id.clone());
        ids.push(listed.id.to_uppercase());
        ids.push(listed.id.replace(['-', '_'], ""));
        ids.push(format!("{}_", listed.id.replace('-', "_")));
    }
    ids.extend(["newapi", "nope", "OpenCodeGo", "open_code-go", ""].map(str::to_string));
    let mut urls = vec![None, some("https://relay.example.invalid/v1"), some("")];
    for api in catalog.providers().filter_map(|listed| listed.api.clone()) {
        let bare = api
            .trim_end_matches('/')
            .trim_end_matches("/v1")
            .to_string();
        urls.push(Some(api.clone()));
        urls.push(Some(format!("{api}/")));
        urls.push(Some(format!("{bare}/v1/")));
        urls.push(Some(bare.to_uppercase()));
    }
    let written = [
        None,
        some("deepseek"),
        some("anthropic"),
        some("nope"),
        some(""),
    ];
    let mut asks = Vec::new();
    for id in &ids {
        for url in &urls {
            for catalog in &written {
                asks.push((id.clone(), url.clone(), catalog.clone()));
            }
        }
    }
    asks
}

/// 问模型的那一面的输入：认出的那一家（没有、目录里每一家、没有的一家）× 模型名（目录里每一个，加规整过的、大写的、空格
/// 隔开的、加了后缀的前缀变体、带 `/` 的、几个没有的）× 手写的 `catalog`（有的、不存在的、写法不对的、空的）。
fn model_asks(catalog: &Catalog) -> Vec<(Option<String>, String, Option<String>)> {
    let mut recognized = vec![None, some("nope")];
    recognized.extend(catalog.providers().map(|listed| Some(listed.id.clone())));
    let mut models: Vec<String> = [
        "custom-7b",
        "custom",
        "x",
        "gpt-5-minimal",
        "DeepSeek V4 Flash",
        "deepseek-broken",
        "",
        "fast",
    ]
    .map(str::to_string)
    .to_vec();
    for listed in catalog.providers() {
        for name in listed.models.keys() {
            models.push(name.to_string());
            models.push(normalize(name));
            models.push(name.to_uppercase());
            models.push(name.replace('-', " "));
            models.push(format!("{name}-expire-xxxx"));
            models.push(format!("relay/{name}"));
        }
    }
    let written = [
        None,
        some("deepseek/deepseek-flash"),
        some("deepseek/nope"),
        some("nope"),
        some(""),
    ];
    let mut asks = Vec::new();
    for provider in &recognized {
        for model in &models {
            for catalog in &written {
                asks.push((provider.clone(), model.clone(), catalog.clone()));
            }
        }
    }
    asks
}

/// 照 `lookup` 问一遍全部输入，每一个都和 `expected`（整份目录现算的）逐字比。
fn ask_all(
    lookup: CatalogLookup<'_>,
    providers: &[(String, Option<String>, Option<String>)],
    expected_providers: &[Option<ProviderSide>],
    models: &[(Option<String>, String, Option<String>)],
    expected_models: &[ModelSide],
    pass: &str,
) {
    let vendors = vendors();
    for ((id, url, written), expected) in providers.iter().zip(expected_providers) {
        let got = lookup.provider_side(id, url.as_deref(), written.as_deref());
        same(expected, &got, &|| {
            format!("{pass}：供应商 {id:?} {url:?} {written:?}")
        });
    }
    for ((recognized, model, written), expected) in models.iter().zip(expected_models) {
        let got = lookup.model_side(&vendors, recognized.as_deref(), model, written.as_deref());
        same(expected, &got, &|| {
            format!("{pass}：模型 {recognized:?} {model:?} {written:?}")
        });
    }
}

fn locked(recalled: &Mutex<Recalled>) -> std::sync::MutexGuard<'_, Recalled> {
    recalled.lock().unwrap_or_else(PoisonError::into_inner)
}

#[test]
fn recalled_answers_word_for_word_like_the_full_catalog() {
    let loaded = loaded();
    let vendors = vendors();
    let full = CatalogLookup::Full(&loaded);
    let providers = provider_asks(&loaded.catalog);
    let models = model_asks(&loaded.catalog);
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
    // 输入要覆盖到每一种答：认出来的（四种认法都有）、认不出的；四层对上的、手写指定不存在的、没对上的。
    let hows: BTreeSet<&str> = expected_providers
        .iter()
        .flatten()
        .map(|side| side.recognized.how.as_str())
        .collect();
    assert_eq!(hows.len(), 4, "四种认法都要问到：{hows:?}");
    assert!(expected_providers.iter().any(Option::is_none));
    let layers: BTreeSet<u8> = expected_models
        .iter()
        .filter_map(|side| match &side.found {
            crate::matching::Found::Matched(matched) => Some(matched.layer),
            _ => None,
        })
        .collect();
    assert_eq!(layers, BTreeSet::from([1, 2, 3, 4]), "四层都要问到");
    assert!(
        expected_models
            .iter()
            .any(|side| matches!(side.found, crate::matching::Found::Missing(_)))
    );
    assert!(
        expected_models
            .iter()
            .any(|side| side.found == crate::matching::Found::Nothing)
    );

    let recalled = Mutex::new(Recalled::new("trimmed 1", loaded.source, &loaded.fetched));
    let filling = CatalogLookup::Recalled(Recalling {
        recalled: &recalled,
        catalog: Some(&loaded),
        remember: true,
        missed: None,
    });
    ask_all(
        filling,
        &providers,
        &expected_providers,
        &models,
        &expected_models,
        "边查边记",
    );
    // 每个不一样的输入都记了一条，负的答也记。
    let unique_providers: BTreeSet<_> = providers.iter().collect();
    let unique_models: BTreeSet<_> = models.iter().collect();
    assert_eq!(
        locked(&recalled).len(),
        (unique_providers.len(), unique_models.len())
    );

    // 不借目录：答得出来、答得一样，就是每一条都照记下的答的。
    let recalling = CatalogLookup::Recalled(Recalling {
        recalled: &recalled,
        catalog: None,
        remember: true,
        missed: None,
    });
    ask_all(
        recalling,
        &providers,
        &expected_providers,
        &models,
        &expected_models,
        "只照记下的",
    );

    // 写成 JSON、读回来，一模一样，照它查也一样。
    let text = locked(&recalled).to_json().expect("写得成");
    let back = Recalled::from_json(&text).expect("读得回来");
    assert_eq!(back, *locked(&recalled));
    assert_eq!(back.of(), "trimmed 1");
    let back = Mutex::new(back);
    let reread = CatalogLookup::Recalled(Recalling {
        recalled: &back,
        catalog: None,
        remember: true,
        missed: None,
    });
    ask_all(
        reread,
        &providers,
        &expected_providers,
        &models,
        &expected_models,
        "读回来的",
    );
    for lookup in [full, filling, recalling, reread] {
        assert_eq!(lookup.fetched(), loaded.fetched);
        assert_eq!(lookup.source(), loaded.source);
    }
}

#[test]
fn asking_without_remembering_answers_the_same_and_keeps_nothing() {
    let loaded = loaded();
    let vendors = vendors();
    let full = CatalogLookup::Full(&loaded);
    let recalled = Mutex::new(Recalled::new("trimmed 1", loaded.source, &loaded.fetched));
    let passing = CatalogLookup::Recalled(Recalling {
        recalled: &recalled,
        catalog: Some(&loaded),
        remember: false,
        missed: None,
    });
    for (id, url, written) in provider_asks(&loaded.catalog) {
        same(
            &full.provider_side(&id, url.as_deref(), written.as_deref()),
            &passing.provider_side(&id, url.as_deref(), written.as_deref()),
            &|| format!("供应商 {id:?}"),
        );
    }
    for (recognized, model, written) in model_asks(&loaded.catalog).into_iter().take(500) {
        same(
            &full.model_side(&vendors, recognized.as_deref(), &model, written.as_deref()),
            &passing.model_side(&vendors, recognized.as_deref(), &model, written.as_deref()),
            &|| format!("模型 {model:?}"),
        );
    }
    assert_eq!(locked(&recalled).len(), (0, 0), "不记的一条都不留");

    // 记过的照样照记下的答，不记的开关只管没记过的。
    let mut recalled = Recalled::new("trimmed 1", loaded.source, &loaded.fetched);
    let first = recalled.provider_side(Some(&loaded), "deepseek", None, None, true);
    assert!(matches!(first, Recall::Known(Some(_))));
    assert_eq!(
        recalled.provider_side(None, "deepseek", None, None, false),
        first
    );
    assert_eq!(recalled.len(), (1, 0));
}

#[test]
fn without_a_catalog_to_borrow_an_unknown_input_is_unknown() {
    let loaded = loaded();
    let vendors = vendors();
    let mut recalled = Recalled::new("trimmed 1", loaded.source, &loaded.fetched);
    assert_eq!(
        recalled.provider_side(None, "deepseek", None, None, true),
        Recall::Unknown
    );
    assert_eq!(
        recalled.model_side(
            None,
            &vendors,
            Some("deepseek"),
            "deepseek-flash",
            None,
            true
        ),
        Recall::Unknown
    );
    assert_eq!(recalled.len(), (0, 0), "说不出来的不记");
    // 认不出、没对上的负的答记下了，没有目录也答得出来。
    recalled.provider_side(Some(&loaded), "newapi", None, None, true);
    recalled.model_side(Some(&loaded), &vendors, None, "custom-7b", None, true);
    assert_eq!(
        recalled.provider_side(None, "newapi", None, None, true),
        Recall::Known(None)
    );
    assert_eq!(
        recalled.model_side(None, &vendors, None, "custom-7b", None, true),
        Recall::Known(ModelSide::nothing())
    );
    // 合资料时说不出来的当没有目录。
    let empty = Mutex::new(Recalled::new("trimmed 1", loaded.source, &loaded.fetched));
    let lookup = CatalogLookup::Recalled(Recalling {
        recalled: &empty,
        catalog: None,
        remember: true,
        missed: None,
    });
    assert_eq!(lookup.provider_side("deepseek", None, None), None);
    assert_eq!(
        lookup.model_side(&vendors, Some("deepseek"), "deepseek-flash", None),
        ModelSide::nothing()
    );
}

#[test]
fn a_recall_that_cannot_be_read_says_why() {
    let error = Recalled::from_json("{").expect_err("读不懂");
    assert!(error.starts_with("catalog recall not readable"), "{error}");
    let loaded = loaded();
    let text = Recalled::new("trimmed 1", loaded.source, &loaded.fetched)
        .to_json()
        .expect("写得成");
    let older = text.replacen(&format!("{{\"v\":{RECALL_VERSION},"), "{\"v\":0,", 1);
    assert_ne!(older, text);
    let error = Recalled::from_json(&older).expect_err("版本不对");
    assert!(error.contains("version 0"), "{error}");
    let error = Recalled::from_json("{\"v\":1}").expect_err("缺格");
    assert!(error.starts_with("catalog recall not readable"), "{error}");
}

/// 合资料用的配置：目录里每一家各配一家（不另写什么；编号数字打头的换个编号、写上那一家的地址），另配几家走别的认法的：去掉分隔认的、照地址认的、手写 `catalog`
/// 的、认不出的（自己写了地址的、没写地址的），再给中转的两个模型手写 `catalog`（有的、不存在的）。
fn configured(catalog: &Catalog) -> (String, Vec<String>) {
    let mut source = String::new();
    let mut ids = Vec::new();
    for listed in catalog.providers() {
        // 编号、`catalog` 都不许数字打头（`302ai`）：换个编号，照目录里那一家的地址认。
        match (
            listed.id.starts_with(|c: char| c.is_ascii_digit()),
            &listed.api,
        ) {
            (true, None) => {}
            (true, Some(api)) => {
                let id = format!("p{}", listed.id);
                source.push_str(&format!(
                    "[providers.{id}]\nlocal = false\nbase_url = \"{api}\"\n\n"
                ));
                ids.push(id);
            }
            (false, _) => {
                source.push_str(&format!("[providers.{}]\nlocal = false\n\n", listed.id));
                ids.push(listed.id.clone());
            }
        }
    }
    source.push_str(
        "[providers.opencodego]\nlocal = false\n\n\
         [providers.proxy]\nbase_url = \"HTTPS://API.DEEPSEEK.COM/v1/\"\n\n\
         [providers.custom]\nbase_url = \"https://custom.invalid\"\ncatalog = \"anthropic\"\n\n\
         [providers.ghost]\ncatalog = \"nope\"\n\n\
         [providers.newapi]\nbase_url = \"https://relay.example.invalid/v1\"\n\n\
         [providers.newapi.models.\"x\"]\ncatalog = \"deepseek/nope\"\n\n\
         [providers.newapi.models.\"y\"]\ncatalog = \"openai/gpt-5\"\n",
    );
    ids.extend(["opencodego", "proxy", "custom", "ghost", "newapi"].map(str::to_string));
    (source, ids)
}

/// 每一家配好的、每个模型名（目录里每一个，加几个别的写法）：两种办法下合出来的供应商、模型的资料（连同 `model.list` 的
/// 写法）、思考强度的档位逐字一样。
#[test]
fn facts_and_providers_are_the_same_either_way() {
    let held = Held::new(
        serde_json::json!({"npm": {
            "@ai-sdk/openai-compatible": "openai-chat",
            "@ai-sdk/anthropic": "anthropic",
            "@ai-sdk/openai": "openai-responses"
        }}),
        true,
    );
    let Some(loaded) = held.catalog.as_ref() else {
        panic!("带着目录");
    };
    let (source, ids) = configured(&loaded.catalog);
    let resolved = resolved(&source);
    let values = resolved.values();
    let mut models: Vec<String> = [
        "x",
        "y",
        "custom-7b",
        "DeepSeek V4 Flash",
        "deepseek-flash-expire-xxxx",
    ]
    .map(str::to_string)
    .to_vec();
    for listed in loaded.catalog.providers() {
        models.extend(listed.models.keys().map(|name| name.to_string()));
    }
    let file = |_: Layer| "config.toml".to_string();
    let full = held.knowledge();

    let recalled = Mutex::new(Recalled::new("trimmed 1", loaded.source, &loaded.fetched));
    let check = |knowledge: &Knowledge<'_>, pass: &str| {
        let mut usable = 0;
        for id in &ids {
            let expected = provider(&values, &full, id);
            let got = provider(&values, knowledge, id);
            same(&expected, &got, &|| format!("{pass}：供应商 {id}"));
            let (Ok(expected), Ok(got)) = (expected, got) else {
                continue;
            };
            usable += 1;
            for model in &models {
                let want = facts(&resolved, &full, &expected, model);
                let have = facts(&resolved, knowledge, &got, model);
                same(&want, &have, &|| format!("{pass}：{id}/{model} 的资料"));
                assert_eq!(
                    want.0.json(&file),
                    have.0.json(&file),
                    "{pass}：{id}/{model}"
                );
                let text = format!("{id}/{model}");
                same(
                    &levels_of(&resolved, &full, &text),
                    &levels_of(&resolved, knowledge, &text),
                    &|| format!("{pass}：{text} 的档位"),
                );
            }
        }
        assert!(
            usable > ids.len() / 2,
            "{pass}：大多数家要用得上，才比得出资料"
        );
        assert!(usable < ids.len(), "{pass}：也要有用不了的一家");
    };
    check(&recalling(full, Some(loaded), &recalled), "边查边记");
    check(&recalling(full, None, &recalled), "只照记下的");
    let text = locked(&recalled).to_json().expect("写得成");
    let back = Mutex::new(Recalled::from_json(&text).expect("读得回来"));
    check(&recalling(full, None, &back), "读回来的");
}

/// 同 `full`，只是照 `recalled` 记下的答查目录，没记过的借 `catalog`。
fn recalling<'a>(
    full: Knowledge<'a>,
    catalog: Option<&'a Loaded>,
    recalled: &'a Mutex<Recalled>,
) -> Knowledge<'a> {
    Knowledge {
        lookup: Some(CatalogLookup::Recalled(Recalling {
            recalled,
            catalog,
            remember: true,
            missed: None,
        })),
        ..full
    }
}

mod rebuild;
