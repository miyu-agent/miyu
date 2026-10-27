//! 配置检查报的软件源（施工 U23 下下）：叫 `official` 的、只写了一半的、公钥读不成的，个人设置里和系统配置同名的；写全了、
//! 公钥读得成的不报。

use miyu_config::parse::parse;

use super::*;
use crate::settings::SOURCE_FEEDS;

/// 一把读得成的公钥（32 个零字节）。
const KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

fn parsed(layer: Layer, text: &str) -> Parsed {
    parse(SOURCE_FEEDS, layer, text).unwrap_or_else(|problem| panic!("读得成：{problem:?}"))
}

/// 报出来的：原因码、名字，照名字排。
fn said(problems: &[Problem]) -> Vec<(Code, String)> {
    problems
        .iter()
        .map(|problem| (problem.code, problem.name.clone().unwrap_or_default()))
        .collect()
}

#[test]
fn bad_sources_are_reported_and_good_ones_are_not() {
    let text = format!(
        "[packages.sources.official]\nurl = \"https://a.example/\"\nkey = \"{KEY}\"\n\n\
         [packages.sources.good]\nurl = \"https://b.example/\"\nkey = \"{KEY}\"\n\n\
         [packages.sources.half]\nurl = \"https://c.example/\"\n\n\
         [packages.sources.wrong]\nurl = \"https://d.example/\"\nkey = \"not-a-key\"\n"
    );
    let system = parsed(Layer::System, &text);
    assert_eq!(
        said(&problems(&system, Layer::System, None)),
        [
            (Code::SourceIncomplete, "half".to_string()),
            (Code::SourceReserved, "official".to_string()),
            (Code::SourceKey, "wrong".to_string()),
        ]
    );
    let first = &problems(&system, Layer::System, None)[0];
    assert_eq!(
        first.key.as_deref(),
        Some("packages.sources.half.url"),
        "指到写它的那一项"
    );
}

#[test]
fn a_personal_source_named_like_a_system_one_is_reported() {
    let system = parsed(
        Layer::System,
        &format!("[packages.sources.shared]\nurl = \"https://a.example/\"\nkey = \"{KEY}\"\n"),
    );
    let personal = parsed(
        Layer::Personal,
        &format!(
            "[packages.sources.shared]\nurl = \"https://b.example/\"\nkey = \"{KEY}\"\n\n\
             [packages.sources.mine]\nurl = \"https://c.example/\"\nkey = \"{KEY}\"\n"
        ),
    );
    let found = problems(&personal, Layer::Personal, Some(&system));
    assert_eq!(said(&found), [(Code::SourceShadowed, "shared".to_string())]);
    assert_eq!(
        found[0].code.severity(),
        miyu_config::problem::Severity::Warning
    );
    assert!(
        problems(&system, Layer::System, Some(&system)).is_empty(),
        "系统配置自己不和自己比"
    );
}
