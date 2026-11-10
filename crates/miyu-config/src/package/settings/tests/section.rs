//! 组的名字和 `shown_when`（施工 SG-1）：读对、照写的先后；一个值的也读成列表；指的项不在这个包里、是别的包的键、写全了前缀、
//! 不是选项、在这一组里面、值不是它的选项，都报 `bad_shown_when`；没分组的包没有节。

use super::*;

/// 一个选引擎的选项，两个照它画的组，一个不写名字、不写条件的组。
const ENGINES: &str = r#"[settings.engine]
type = "option"
choices = ["minimax", "mimo", "openai"]
default = "minimax"
name = { en = "Engine" }

[settings.minimax]
name = { en = "MiniMax", zh = "MiniMax 语音" }
shown_when = { engine = "minimax" }

[settings.minimax.key]
type = "secret"
name = { en = "Key" }

[settings.cloud]
name = { en = "Cloud" }
shown_when = { "engine" = ["mimo", "openai"] }

[settings.cloud.voice]
type = "text"
name = { en = "Voice" }

[settings.extra.speed]
type = "int"
name = { en = "Speed" }
"#;

#[test]
fn a_group_reads_its_name_and_when_it_shows() {
    let manifest = read(&web(ENGINES)).unwrap();
    let names: Vec<(&str, Option<&str>)> = manifest
        .settings
        .iter()
        .map(|one| (one.name.as_str(), one.section()))
        .collect();
    assert_eq!(
        names,
        [
            ("engine", None),
            ("minimax.key", Some("minimax")),
            ("cloud.voice", Some("cloud")),
            ("extra.speed", Some("extra")),
        ],
        "组自己的两格不当成项"
    );
    let sections = &manifest.sections;
    let ids: Vec<&str> = sections.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids, ["minimax", "cloud", "extra"], "照写的先后");
    assert_eq!(
        sections[0].name.get("zh").map(String::as_str),
        Some("MiniMax 语音")
    );
    let when = |index: usize| {
        let when = sections[index].shown_when.as_ref().expect("写了");
        (when.setting.as_str(), when.values.clone())
    };
    assert_eq!(
        when(0),
        ("engine", vec!["minimax".to_string()]),
        "一个值的也是列表"
    );
    assert_eq!(
        when(1),
        ("engine", vec!["mimo".to_string(), "openai".to_string()])
    );
    assert!(sections[2].name.is_empty(), "没写名字的是空的");
    assert_eq!(sections[2].shown_when, None, "没写条件的总是画");
}

#[test]
fn a_package_without_groups_has_no_sections() {
    let manifest = read(&web(PORT)).unwrap();
    assert!(manifest.sections.is_empty());
}

#[test]
fn a_wrong_shown_when_is_a_problem() {
    let with = |when: &str| {
        ENGINES.replace(
            "shown_when = { engine = \"minimax\" }",
            &format!("shown_when = {when}"),
        )
    };
    let cases = [
        ("{ nope = \"minimax\" }", "这个包里没有这一项"),
        ("{ \"other.engine\" = \"minimax\" }", "别的包的键"),
        ("{ \"web.engine\" = \"minimax\" }", "写全了前缀的不认"),
        ("{ \"cloud.voice\" = \"minimax\" }", "不是选项"),
        ("{ \"minimax.key\" = \"minimax\" }", "在这一组里面"),
        ("{ engine = \"azure\" }", "值不是它的选项"),
        ("{ engine = [] }", "空的列表"),
        ("{ engine = 1 }", "不是字"),
        ("{ engine = \"minimax\", other = \"x\" }", "两项"),
        ("{}", "空的表"),
        ("\"engine\"", "不是表"),
    ];
    for (when, why) in cases {
        let problem = read(&web(&with(when))).expect_err(why);
        assert_eq!(problem.code, Code::BadShownWhen, "{why}：{problem:?}");
        assert_eq!(problem.detail, "settings.minimax.shown_when", "{why}");
        assert!(problem.line.is_some(), "{why}：报在那一行");
    }
}

#[test]
fn a_group_name_is_read_like_an_item_name() {
    assert_eq!(
        wrong(
            "[settings.g]\nname = \"G\"\n\n[settings.g.a]\ntype = \"bool\"\nname = { en = \"A\" }\n"
        ),
        Code::NotPhrases,
        "写了名字、底下有项的是一组，名字照一项的写法查"
    );
    assert_eq!(
        wrong("[settings.g]\nname = { en = \"G\" }\ndescription = { en = \"D\" }\n"),
        Code::MissingKey,
        "只有名字、说明的是缺了 type 的一项"
    );
}
