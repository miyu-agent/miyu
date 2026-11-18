//! 不认识的键，值写成行内表的（施工 AUD-6）：行内表是写在一行里的一个值（引用密钥、环境变量就是这样写的），当一个值报
//! 人写的那个键一次，不往里钻。原来钻进去，一格报一条、报的键名多一层（`providers.dev.kee.secret`），不是人写的那个键。
//! 标准的表头、点号连着写的照旧一项一条；认识的一组键写成行内表的照旧往里读。

use super::*;

/// 这份配置的字读出来的问题：种类、键、位置。
fn problems(source: &str) -> Vec<(Code, Option<String>, Option<At>)> {
    parse(&items(), Layer::System, source)
        .expect("写法对")
        .problems
        .into_iter()
        .map(|problem| (problem.code, problem.key, problem.at))
        .collect()
}

#[test]
fn an_unknown_key_holding_an_inline_table_is_reported_once_as_written() {
    let source = "[ui]\nlanguage = \"zh\"\nnosuch = { secret = \"x\", extra = 1 }\n\n[weird]\nthing = { env = \"X\" }\n";
    assert_eq!(
        problems(source),
        [
            (
                Code::UnknownKey,
                Some("ui.nosuch".to_string()),
                Some(at(3, 1))
            ),
            (
                Code::UnknownKey,
                Some("weird.thing".to_string()),
                Some(at(6, 1))
            ),
        ]
    );
}

#[test]
fn a_known_group_written_inline_is_still_read_into() {
    let parsed = parse(
        &items(),
        Layer::System,
        "ui = { language = \"zh\", langauge = \"en\" }\n",
    )
    .expect("写法对");
    assert_eq!(parsed.entries.len(), 1, "认识的那一项照收");
    let keys: Vec<_> = parsed
        .problems
        .iter()
        .map(|problem| (problem.code, problem.key.as_deref(), problem.suggest))
        .collect();
    assert_eq!(
        keys,
        [(Code::UnknownKey, Some("ui.langauge"), Some("ui.language"))],
        "一组键里拼错的照旧一项一条、找得到最近的"
    );
}

#[test]
fn unknown_tables_with_headers_or_dots_still_report_each_key() {
    let source = "[weird]\nthing = 1\nother = 2\n\n[empty]\nuii.language = 1\n";
    let keys: Vec<_> = problems(source)
        .into_iter()
        .map(|(_, key, _)| key.unwrap_or_default())
        .collect();
    assert_eq!(keys, ["weird.thing", "weird.other", "empty.uii.language"]);
}
