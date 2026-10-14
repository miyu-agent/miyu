//! 目录怎么读（施工 8-7）：真目录裁出来的一份读得进、坏的模型跳过、不认识的格不理、`limit.input` 比 `context` 小的取小的。

use serde_json::json;

use super::*;
use crate::test_support::TRIMMED;

#[test]
fn the_trimmed_real_catalog_reads_with_the_broken_model_skipped() {
    let read = Catalog::parse(TRIMMED).expect("读得进");
    assert_eq!(read.skipped, ["deepseek/deepseek-broken"]);
    let catalog = read.catalog;
    assert_eq!(catalog.providers().count(), 10);
    assert_eq!(catalog.model_count(), 22);
    let deepseek = catalog.provider("deepseek").expect("有 DeepSeek");
    assert_eq!(deepseek.name.as_deref(), Some("DeepSeek"));
    assert_eq!(deepseek.env, ["DEEPSEEK_API_KEY"]);
    assert_eq!(deepseek.npm.as_deref(), Some("@ai-sdk/openai-compatible"));
    assert_eq!(deepseek.api.as_deref(), Some("https://api.deepseek.com"));
    assert!(deepseek.doc.is_some());
    // 用得上的格一格格读对；不认识的（`description`、`release_date` 这些）不理。
    let flash = catalog.model("deepseek", "deepseek-flash").expect("有");
    assert_eq!(flash.name.as_deref(), Some("DeepSeek V4.1 Flash"));
    assert_eq!(flash.family.as_deref(), Some("deepseek-flash"));
    assert_eq!(flash.tools, Some(true));
    assert_eq!(
        flash
            .inputs
            .map(|inputs| inputs.names().collect::<Vec<_>>()),
        Some(vec!["text", "image"])
    );
    assert_eq!(
        (flash.window, flash.max_output),
        (Some(1_000_000), Some(393_216))
    );
    assert_eq!(
        flash.reasoning.as_deref().cloned(),
        Some(Reasoning {
            levels: vec!["low".to_string(), "high".to_string(), "max".to_string()],
            toggle: true,
        })
    );
    assert_eq!(flash.status, None);
    let price = flash.price.as_ref().expect("有价");
    assert_eq!(price.rates.input, Some(0.15));
    assert_eq!(price.rates.cache_write, None);
    assert_eq!(price.reasoning, Some(0.6));
    assert_eq!(price.currency, "USD");
    assert_eq!(
        catalog
            .model("deepseek", "deepseek-v4-flash")
            .and_then(|model| model.status.as_deref()),
        Some("deprecated")
    );
    // 窗口取 `limit.context` 和 `limit.input` 里小的那个。
    let hy3 = catalog.model("deepinfra", "tencent/Hy3").expect("有");
    assert_eq!((hy3.window, hy3.max_output), (Some(192_000), Some(128_000)));
    assert_eq!(hy3.reasoning, None, "空的 reasoning_options 是没有");
    // 分档、超过 20 万的价。
    let sonnet = catalog.model("aihubmix", "claude-sonnet-4-5").expect("有");
    let price = sonnet.price.as_ref().expect("有价");
    assert_eq!(price.tiers.len(), 1);
    assert_eq!(price.tiers[0].size, 200_000);
    assert_eq!(price.tiers[0].rates.input, Some(6.6));
    assert_eq!(price.over_200k.and_then(|rates| rates.output), Some(24.75));
}

#[test]
fn the_index_finds_names_and_normalized_names_in_byte_order() {
    let catalog = Catalog::parse(TRIMMED).expect("读得进").catalog;
    let named: Vec<&str> = catalog
        .named("deepseek-v4.1-flash")
        .iter()
        .map(|(provider, _)| &**provider)
        .collect();
    assert_eq!(named, ["above", "aihubmix", "opencode", "opencode-go"]);
    assert_eq!(catalog.normalized("deepseek-v4-1-flash").len(), 4);
    let hy3: Vec<(&str, &str)> = catalog
        .normalized("hy3")
        .iter()
        .map(|(provider, model)| (&**provider, &**model))
        .collect();
    assert_eq!(hy3, [("deepinfra", "tencent/Hy3")]);
    assert!(catalog.named("nope").is_empty());
}

#[test]
fn odd_fields_are_read_the_cautious_way() {
    let text = json!({
        "p": {"id": "p", "models": {
            "toggle": {"reasoning_options": [{"type": "toggle"}], "modalities": {"input": ["video", "text", "audio", "smell", "pdf"]}},
            "null-level": {"reasoning_options": [{"type": "effort", "values": [null, "low", "high"]}]},
            "off-by-name": {"reasoning_options": [{"type": "effort", "values": ["none", "disabled", "low"]}]},
            "budget-only": {"reasoning_options": [{"type": "budget_tokens", "min": 1024}]},
            "input-only": {"limit": {"input": 5000}},
            "zero": {"limit": {"context": 0, "output": 0}},
            "tiered": {"cost": {"input": 1, "tiers": [{"input": 2, "tier": {"type": "other", "size": 9}}]}},
            "bad-limit": {"limit": {"context": -1}},
            "bad-cost": {"cost": {"input": "cheap"}},
            "bad-tools": {"tool_call": "yes"}
        }},
        "q": {"env": "NOT_A_LIST", "models": {}}
    })
    .to_string();
    let read = Catalog::parse(&text).expect("读得进");
    assert_eq!(
        read.skipped,
        ["p/bad-cost", "p/bad-limit", "p/bad-tools", "q"]
    );
    let model = |name: &str| read.catalog.model("p", name).cloned().expect("有");
    assert_eq!(
        model("toggle").reasoning.as_deref().cloned(),
        Some(Reasoning {
            levels: Vec::new(),
            toggle: true
        }),
        "只有开关的：没有档位，开关记下（能不能关照档案，施工 8-18）"
    );
    assert_eq!(
        model("off-by-name").reasoning.as_deref().cloned(),
        Some(Reasoning {
            levels: vec!["off".to_string(), "low".to_string()],
            toggle: false
        }),
        "none、disabled 读成 off，重复的只留第一个（施工 8-18）"
    );
    assert_eq!(
        model("budget-only").reasoning,
        None,
        "只有思考预算的没有档位"
    );
    assert_eq!(
        model("toggle")
            .inputs
            .map(|inputs| inputs.names().collect::<Vec<_>>()),
        Some(vec!["text", "pdf", "audio", "video"]),
        "认得的几种照固定的先后，音频、视频也认（施工 8-27），认不得的不要"
    );
    assert_eq!(
        model("null-level")
            .reasoning
            .map(|reasoning| reasoning.levels.clone()),
        Some(vec!["low".to_string(), "high".to_string()]),
        "写了 null 的那一级不算，整个模型照读"
    );
    assert_eq!(model("input-only").window, Some(5000));
    assert_eq!(
        (model("zero").window, model("zero").max_output),
        (None, None)
    );
    let tiered = model("tiered").price.expect("有价");
    assert!(tiered.tiers.is_empty(), "不认识的档不理");
    assert_eq!(model("tiered").inputs, None);
}

#[test]
fn a_catalog_that_is_not_a_table_of_providers_is_unreadable() {
    for text in ["", "[]", "{\"p\": 1", "not json"] {
        let error = Catalog::parse(text).expect_err("读不了");
        assert!(error.starts_with("catalog not readable: "), "{error}");
    }
    assert_eq!(
        Catalog::parse("{}")
            .expect("空的也读得进")
            .catalog
            .model_count(),
        0
    );
}

/// 模型自己的包名、交错思考（施工 8-14）：`provider.npm`、`interleaved` 写成 `{"field": …}` 的取字段名，`true` 和别的写法当没写。
#[test]
fn a_model_carries_its_own_package_and_interleaved_field() {
    let catalog = Catalog::parse(TRIMMED).expect("读得进").catalog;
    let model = |provider: &str, name: &str| catalog.model(provider, name).expect("有").clone();
    assert_eq!(
        model("opencode-go", "minimax-m3").npm.as_deref(),
        Some("@ai-sdk/anthropic")
    );
    assert_eq!(
        model("opencode-go", "gpt-5.6-luna").npm.as_deref(),
        Some("@ai-sdk/openai")
    );
    assert_eq!(
        model("opencode", "gemini-3-pro").npm.as_deref(),
        Some("@ai-sdk/google")
    );
    let flash = model("opencode-go", "deepseek-v4.1-flash");
    assert_eq!(flash.npm, None, "没写的照这一家的");
    assert_eq!(flash.interleaved.as_deref(), Some("reasoning_content"));
    assert_eq!(model("opencode-go", "minimax-m3").interleaved, None);
    assert_eq!(
        model("aihubmix", "claude-sonnet-4-5").interleaved,
        None,
        "写成 true 的说不出字段"
    );
    let odd = Catalog::parse(
        &json!({"p": {"models": {
            "details": {"interleaved": {"field": "reasoning_details"}},
            "number": {"interleaved": {"field": 3}},
            "no-npm": {"provider": {"api": "https://x.invalid"}}
        }}})
        .to_string(),
    )
    .expect("读得进");
    assert!(odd.skipped.is_empty(), "{:?}", odd.skipped);
    let odd = odd.catalog;
    assert_eq!(
        odd.model("p", "details")
            .expect("有")
            .interleaved
            .as_deref(),
        Some("reasoning_details"),
        "认不认交给合资料的一方"
    );
    assert_eq!(odd.model("p", "number").expect("有").interleaved, None);
    assert_eq!(odd.model("p", "no-npm").expect("有").npm, None);
}

#[test]
fn the_same_text_is_kept_once() {
    // 同样的字只存一份（施工 V-2 下）：两家列了同一个模型，表里的键和索引里的条目是同一份；家族照字共用。
    let catalog = Catalog::parse(TRIMMED).expect("读得进").catalog;
    let entries = catalog.named("deepseek-v4.1-flash");
    let (provider, model) = &entries[0];
    let key = catalog
        .provider(provider)
        .and_then(|entry| entry.models.keys().find(|name| ***name == **model))
        .expect("在表里");
    assert!(
        std::sync::Arc::ptr_eq(key, model),
        "索引里的名字就是表里的键"
    );
    let families: Vec<_> = entries
        .iter()
        .filter_map(|(provider, model)| catalog.model(provider, model)?.family.clone())
        .collect();
    assert!(families.len() >= 2, "{families:?}");
    assert!(
        families
            .windows(2)
            .all(|pair| *pair[0] != *pair[1] || std::sync::Arc::ptr_eq(&pair[0], &pair[1])),
        "同样的家族是同一份"
    );
}

#[test]
fn inputs_are_switches() {
    let inputs = super::Inputs::of(["video", "text", "smell", "image"]);
    assert_eq!(
        inputs.names().collect::<Vec<_>>(),
        ["text", "image", "video"]
    );
    assert!(inputs.has("image") && !inputs.has("pdf") && !inputs.has("smell"));
    assert!(super::Inputs::of([]).is_empty());
}

#[test]
fn prices_and_reasoning_written_the_same_are_kept_once() {
    // 施工 V-5 中：原文写得一样的价格、思考强度共用一份，写得不一样的各一份；读不懂的照旧跳过这个模型。
    let cost = json!({"input": 1, "output": 2});
    let effort = json!([{"type": "effort", "values": ["low", "high"]}]);
    let text = json!({"p": {"models": {
        "a": {"cost": cost, "reasoning_options": effort},
        "b": {"cost": cost, "reasoning_options": effort},
        "c": {"cost": {"input": 3}, "reasoning_options": [{"type": "toggle"}]},
        "d": {"cost": {"input": "much"}},
        "e": {"reasoning_options": "fast"},
    }}})
    .to_string();
    let read = Catalog::parse(&text).expect("读得进");
    assert_eq!(read.skipped, ["p/d", "p/e"]);
    let model = |name: &str| read.catalog.model("p", name).expect("有");
    let (a, b, c) = (model("a"), model("b"), model("c"));
    let same = |x: &Option<std::sync::Arc<Price>>, y: &Option<std::sync::Arc<Price>>| {
        std::sync::Arc::ptr_eq(x.as_ref().expect("有价"), y.as_ref().expect("有价"))
    };
    assert!(same(&a.price, &b.price), "写得一样的价格是同一份");
    assert!(!same(&a.price, &c.price), "写得不一样的各一份");
    assert_eq!(
        c.price.as_ref().and_then(|price| price.rates.input),
        Some(3.0)
    );
    let (ra, rb, rc) = (
        a.reasoning.as_ref().expect("有档"),
        b.reasoning.as_ref().expect("有档"),
        c.reasoning.as_ref().expect("有开关"),
    );
    assert!(std::sync::Arc::ptr_eq(ra, rb), "写得一样的思考强度是同一份");
    assert_eq!(ra.levels, ["low", "high"]);
    assert!(rc.toggle && rc.levels.is_empty());
}

#[test]
fn a_provider_s_models_are_sorted_and_found_by_name() {
    // 施工 V-5 中：模型表是排好的一排，照名字二分；`for … in &表` 照名字的字节排。
    let text = json!({"p": {"models": {"b-2": {}, "a-1": {}, "B-0": {}}}}).to_string();
    let catalog = Catalog::parse(&text).expect("读得进").catalog;
    let models = &catalog.provider("p").expect("有").models;
    assert_eq!(models.len(), 3);
    assert_eq!(
        models.keys().map(|name| &**name).collect::<Vec<_>>(),
        ["B-0", "a-1", "b-2"]
    );
    assert!(models.contains_key("a-1") && !models.contains_key("a"));
    assert_eq!(models.into_iter().count(), 3);
    let table = ModelTable::of(vec![
        (Arc::from("x"), CatalogModel::default()),
        (
            Arc::from("x"),
            CatalogModel {
                tools: Some(true),
                ..CatalogModel::default()
            },
        ),
    ]);
    assert_eq!(table.len(), 1, "同名的只留一个");
    assert_eq!(
        table.get("x").and_then(|model| model.tools),
        Some(true),
        "留后写的"
    );
}

#[test]
fn a_digest_reads_back_the_same_catalog_and_only_for_its_own_fingerprint() {
    // 施工 V-5 中：整理好的一份读回来和现解的一模一样；指纹、版本对不上的，读不懂、编号越界的都当没有。
    let read = Catalog::parse(TRIMMED).expect("读得进").catalog;
    let digest = read.digest("snapshot 1 2");
    let back = Catalog::from_digest(&digest, "snapshot 1 2").expect("读得回来");
    assert_eq!(back, read);
    assert_eq!(
        back.named("deepseek-v4.1-flash"),
        read.named("deepseek-v4.1-flash")
    );
    let flash = back.model("deepseek", "deepseek-flash").expect("有");
    let price = flash.price.as_ref().expect("有价");
    assert!(
        matches!(price.currency, std::borrow::Cow::Borrowed(USD)),
        "币种照旧不另分配"
    );
    assert_eq!(
        Catalog::from_digest(&digest, "snapshot 1 3"),
        None,
        "指纹不对"
    );
    let other = digest.replacen("{\"v\":2,", "{\"v\":1,", 1);
    assert_ne!(other, digest);
    assert_eq!(
        Catalog::from_digest(&other, "snapshot 1 2"),
        None,
        "版本不对"
    );
    assert_eq!(Catalog::from_digest("{", "snapshot 1 2"), None, "读不懂");
    let empty_texts = digest.replacen("\"texts\":[", "\"texts\":[],\"x\":[", 1);
    assert_eq!(
        Catalog::from_digest(&empty_texts, "snapshot 1 2"),
        None,
        "编号越界"
    );
}

#[test]
fn a_digest_carries_both_indexes_and_a_broken_one_is_not_trusted() {
    // 施工 V-8：两份索引照平铺的样子写进去，读回来照编号拼，不再现规整、现排序；拼不成的（键的先后乱了、段对不上）
    // 整份当没有，照原文重解。
    let read = Catalog::parse(TRIMMED).expect("读得进").catalog;
    let digest = read.digest("of");
    let value: serde_json::Value = serde_json::from_str(&digest).expect("是 JSON");
    let keys = value["normalized"][0].as_array().expect("有键").len();
    assert!(keys > 1, "规整过的名字的索引写进去了：{keys}");
    let back = Catalog::from_digest(&digest, "of").expect("读得回来");
    let key = crate::matching::normalize("deepseek-v4.1-flash");
    assert!(!read.normalized(&key).is_empty(), "样本里有这个模型");
    assert_eq!(back.normalized(&key), read.normalized(&key));

    let tamper = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut value = value.clone();
        edit(&mut value);
        Catalog::from_digest(&value.to_string(), "of")
    };
    // 每一种坏法只撞一条检查：键的字对调、段不动；挑一段有几个条目的把起点往后挪一位（段不空）；末尾补一个空段。
    let swapped = tamper(&|value| {
        let keys = value["named"][0].as_array_mut().expect("有键");
        let first = keys[0][0].clone();
        keys[0][0] = keys[1][0].clone();
        keys[1][0] = first;
    });
    assert_eq!(swapped, None, "键的先后乱了");
    let several = |value: &serde_json::Value| {
        value["normalized"][0]
            .as_array()
            .expect("有键")
            .iter()
            .position(|key| key[2].as_u64() > key[1].as_u64().map(|from| from + 1))
            .expect("样本里有一个规整名对着几个模型")
    };
    let gap = tamper(&|value| {
        let at = several(value);
        let key = &mut value["normalized"][0][at];
        key[1] = serde_json::json!(key[1].as_u64().expect("是数") + 1);
    });
    assert_eq!(gap, None, "一段的起点接不上前一段");
    let short = tamper(&|value| {
        let keys = value["normalized"][0].as_array_mut().expect("有键");
        keys.pop();
    });
    assert_eq!(short, None, "最后一段没了，条目盖不全");
    let empty = tamper(&|value| {
        // 一个比哪个键都大的字（`~` 排在字母后面），末尾补一个指着它、一个条目都没有的键。
        let texts = value["texts"].as_array_mut().expect("有字");
        texts.push(serde_json::json!("~~~"));
        let bigger = texts.len() - 1;
        let entries = value["named"][1].as_array().expect("有条目").len();
        let keys = value["named"][0].as_array_mut().expect("有键");
        keys.push(serde_json::json!([bigger, entries, entries]));
    });
    assert_eq!(empty, None, "末尾多一个空段");
    let mixed = tamper(&|value| {
        let at = several(value);
        let from = value["normalized"][0][at][1].as_u64().expect("是数") as usize;
        let entries = value["normalized"][1].as_array_mut().expect("有条目");
        entries.swap(from, from + 1);
    });
    assert_eq!(mixed, None, "段里的条目乱了");
}
