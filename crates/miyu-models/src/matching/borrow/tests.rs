//! 中转站的价格照原厂借（施工 PR-1）：中转网关的 `relay/deepseek-v4-1-flash-261017` 对到 byteplus 的同名条目，价格照 DeepSeek
//! 官方的同一个模型借；原厂没列这个版本的照家族借段对得最多的；没价格的不挑；认不出原厂、原厂里什么都对不上的不借。

use serde_json::{Value, json};

use super::super::*;
use crate::test_support::vendors;

/// 照 models.dev 的写法读一份小目录。
fn catalog(json: Value) -> Catalog {
    Catalog::parse(&json.to_string()).expect("读得进").catalog
}

fn flash(input: f64) -> Value {
    json!({"family": "deepseek-flash", "cost": {"input": input, "output": 0.6, "cache_read": 0.003}})
}

/// 网关认不出（目录里没有这一家）的一个模型对到哪、价格借哪一条。
fn borrowed(catalog: &Catalog, model: &str) -> (String, Option<Priced>) {
    match find(catalog, &vendors(), None, model, None) {
        Found::Matched(matched) => (
            format!("{}/{}", matched.provider, matched.model),
            matched.price,
        ),
        other => panic!("没对上：{other:?}"),
    }
}

fn priced(provider: &str, model: &str) -> Option<Priced> {
    Some(Priced::of(provider, model))
}

#[test]
fn a_relay_model_borrows_the_vendor_s_same_model() {
    let catalog = catalog(json!({
        "byteplus": {"models": {"deepseek-v4-1-flash-261017": flash(0.15)}},
        "deepseek": {"models": {
            "deepseek-flash": flash(0.1), "deepseek-v4-flash": flash(0.2), "deepseek-v4.1-flash": flash(0.15)
        }},
    }));
    assert_eq!(
        borrowed(&catalog, "relay/deepseek-v4-1-flash-261017"),
        (
            "byteplus/deepseek-v4-1-flash-261017".to_string(),
            priced("deepseek", "deepseek-v4.1-flash")
        ),
        "条目照旧是 byteplus 的，价格照原厂里规整以后最长的前缀"
    );
}

#[test]
fn without_the_version_the_family_member_sharing_most_segments_lends_it() {
    let catalog = catalog(json!({
        "byteplus": {"models": {"deepseek-v4-1-flash-261017": flash(0.15)}},
        "deepseek": {"models": {
            "deepseek-flash": flash(0.1), "deepseek-v4-flash": flash(0.2),
            "deepseek-v4-flash-vision-exp": flash(0.3),
            "deepseek-v4-pro": {"family": "deepseek-thinking", "cost": {"input": 0.4, "output": 0.8}},
            "deepseek-v4-1-flash-free": {"family": "deepseek-flash"}
        }},
    }));
    assert_eq!(
        borrowed(&catalog, "relay/deepseek-v4-1-flash-261017").1,
        priced("deepseek", "deepseek-v4-flash"),
        "段对得一样多的取字节序在前的；家族不同的、没价格的不挑"
    );
}

#[test]
fn nothing_is_borrowed_without_a_vendor_or_a_match() {
    let unknown = catalog(json!({
        "relay": {"models": {"hy-3": {"family": "hy", "cost": {"input": 1.0, "output": 2.0}}}},
        "tencent": {"models": {"hy-3-pro": {"family": "hy", "cost": {"input": 1.0, "output": 2.0}}}},
    }));
    assert_eq!(
        borrowed(&unknown, "hy-3"),
        ("relay/hy-3".to_string(), None),
        "认不出原厂"
    );
    let unrelated = catalog(json!({
        "byteplus": {"models": {"deepseek-v4-1-flash-261017": flash(0.15)}},
        "deepseek": {"models": {
            "deepseek-v4-pro": {"family": "deepseek-thinking", "cost": {"input": 0.4, "output": 0.8}}
        }},
    }));
    assert_eq!(
        borrowed(&unrelated, "relay/deepseek-v4-1-flash-261017").1,
        None,
        "原厂里名字、家族都对不上"
    );
}
