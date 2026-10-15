//! `[[slash]]`（施工 X-14 下）：每一格读成什么、没写的照什么；每一种写错报 `bad_slash`、报在那一段；要声明 `commands` 能力；
//! 只有扩展包能写；一条一个功能。

use super::super::{Code, read};

/// 一个扩展：声明了 `commands` 能力、两条命令。
const EXTENSION: &str = r#"[package]
protocol = [1, 1]
name = { en = "Weather" }

[command]
name = "weather"
program = "miyu-weather"
about = { en = "Weather" }

[process]
capabilities = ["commands"]

[[slash]]
name = "weather"
summary = { en = "Today's weather", zh = "今天的天气" }
argument = { en = "<city>", zh = "<城市>" }

[[slash]]
name = "forecast"
summary = { en = "Next three days" }
"#;

/// 换掉 `EXTENSION` 里第一段命令的写法。
fn with_first(slash: &str) -> String {
    let start = EXTENSION.find("[[slash]]").expect("有");
    let end = EXTENSION
        .find("[[slash]]\nname = \"forecast\"")
        .expect("有");
    format!("{}{slash}\n{}", &EXTENSION[..start], &EXTENSION[end..])
}

#[test]
fn an_extension_declares_its_commands_with_every_field() {
    let manifest = read(EXTENSION).expect("合写法");
    assert_eq!(manifest.slash.len(), 2);
    let weather = &manifest.slash[0];
    assert_eq!(weather.name, "weather");
    assert_eq!(
        weather.summary.get("zh").map(String::as_str),
        Some("今天的天气")
    );
    assert_eq!(
        weather
            .argument
            .as_ref()
            .and_then(|phrases| phrases.get("en"))
            .map(String::as_str),
        Some("<city>")
    );
    assert_eq!(weather.line, Some(13));
    assert_eq!(weather.feature_id(), "command-weather");
    let forecast = &manifest.slash[1];
    assert_eq!(forecast.argument, None);
}

#[test]
fn each_mistake_is_bad_slash_on_its_table() {
    let cases = [
        (
            "[[slash]]\nname = \"Weather\"\nsummary = { en = \"x\" }\n",
            "slash.name \"Weather\"",
        ),
        ("[[slash]]\nsummary = { en = \"x\" }\n", "slash.name"),
        ("[[slash]]\nname = \"weather\"\n", "slash.weather.summary"),
    ];
    for (slash, detail) in cases {
        let problem = read(&with_first(slash)).expect_err(slash);
        assert_eq!(problem.code, Code::BadSlash, "{slash}: {problem:?}");
        assert_eq!(problem.detail, detail, "{slash}");
        assert_eq!(problem.line, Some(13), "{slash}");
    }
    // 同一个包里重了的报在后写的那一段。
    let twice = read(&with_first(
        "[[slash]]\nname = \"forecast\"\nsummary = { en = \"x\" }\n",
    ))
    .expect_err("重了");
    assert_eq!(
        (twice.code, twice.detail.as_str(), twice.line),
        (Code::BadSlash, "slash.forecast", Some(17))
    );
    let unknown = read(&with_first(
        "[[slash]]\nname = \"weather\"\nsummary = { en = \"x\" }\nprogram = \"x\"\n",
    ))
    .expect_err("不认识的键");
    assert_eq!(unknown.code, Code::UnknownKey);
}

#[test]
fn declaring_commands_needs_the_commands_capability() {
    let without = EXTENSION.replace(
        "capabilities = [\"commands\"]",
        "capabilities = [\"tools\"]",
    );
    let problem = read(&without).expect_err("没声明能力");
    assert_eq!(problem.code, Code::BadSlash);
    assert_eq!(problem.detail, "slash");
    assert_eq!(problem.line, Some(13));
}

#[test]
fn only_an_extension_may_declare_commands() {
    let bundle =
        "[package]\nname = { en = \"Kit\" }\n\n[[slash]]\nname = \"x\"\nsummary = { en = \"x\" }\n";
    assert_eq!(
        read(bundle).expect_err("bundle 不能写").code,
        Code::WrongKind
    );
    let not_tables = EXTENSION.replace(
        "[[slash]]\nname = \"forecast\"",
        "[slash]\nname = \"forecast\"",
    );
    assert!(read(&not_tables).is_err(), "写成一张表的不认");
}
