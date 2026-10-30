//! 一组字段的列表（施工 PC-2，设计 53 第二节）：表的数组、行内的两种写法都读得成；每一项照字段查，问题报到「第几项的哪个字段」；
//! 超过 100 项的整条报；有一处不对的整条不收。

use crate::item::{Item, Kind, Layer};
use crate::parse::parse;
use crate::problem::Code;
use crate::secret::Reference;
use crate::value::Value;

/// 生图包的配置项：一组账号（备注、地址有默认值、密钥）。
pub(crate) fn draw() -> Vec<Item> {
    let manifest = crate::package::read(
        r#"[package]
name = { en = "Draw" }

[settings.accounts]
type = "list"
element = "record"
title = "name"
name = { en = "Accounts" }

[settings.accounts.fields.name]
type = "text"
name = { en = "Name" }

[settings.accounts.fields.base_url]
type = "url"
default = "https://api.example.com/v1"
name = { en = "Endpoint" }

[settings.accounts.fields.key]
type = "secret"
name = { en = "Key" }

[settings.accounts.fields.weight]
type = "int"
min = 1
max = 9
default = 1
name = { en = "Weight" }
"#,
    )
    .expect("清单读得成");
    crate::package::settings::items("draw", "draw", &manifest.settings)
}

fn record(fields: &[(&str, Value)]) -> Value {
    Value::Record(
        fields
            .iter()
            .map(|(name, value)| (name.to_string(), value.clone()))
            .collect(),
    )
}

fn text(value: &str) -> Value {
    Value::Text(value.to_string().into())
}

fn secret(name: &str) -> Value {
    Value::Secret(Reference::Secret(name.to_string()))
}

#[test]
fn both_ways_of_writing_a_record_list_read() {
    let items = draw();
    assert!(matches!(items[0].kind, Kind::List(Kind::Record(_))));
    let tables = "[draw]\n\n# 第一个\n[[draw.accounts]]\nname = \"Home\"\nkey = { secret = \"home\" }\n\n[[draw.accounts]]\nname = \"Work\"\nbase_url = \"https://work.example.com/v1\"\nkey = { env = \"WORK_KEY\" }\nweight = 3\n";
    let inline = "[draw]\naccounts = [{ name = \"Home\", key = { secret = \"home\" } }, { name = \"Work\", base_url = \"https://work.example.com/v1\", key = { env = \"WORK_KEY\" }, weight = 3 }]\n";
    let expected = Value::List(
        vec![
            record(&[("name", text("Home")), ("key", secret("home"))]),
            record(&[
                ("name", text("Work")),
                ("base_url", text("https://work.example.com/v1")),
                ("key", Value::Secret(Reference::Env("WORK_KEY".to_string()))),
                ("weight", Value::Int(3)),
            ]),
        ]
        .into(),
    );
    for text in [tables, inline] {
        let parsed = parse(&items, Layer::System, text).expect("读得懂");
        assert!(parsed.problems.is_empty(), "{text}：{:?}", parsed.problems);
        assert_eq!(parsed.entries["draw.accounts"].value, expected, "{text}");
    }
    let empty = parse(&items, Layer::System, "[draw]\naccounts = []\n").expect("读得懂");
    assert_eq!(
        empty.entries["draw.accounts"].value,
        Value::List(Vec::new().into()),
        "空的也收"
    );
}

#[test]
fn every_wrong_record_is_reported_at_its_field() {
    let items = draw();
    let problems = |text: &str| {
        let parsed = parse(&items, Layer::System, text).expect("读得懂");
        assert!(
            !parsed.entries.contains_key("draw.accounts"),
            "有错的整条不收：{text}"
        );
        parsed
            .problems
            .iter()
            .map(|problem| (problem.key.clone().unwrap_or_default(), problem.code))
            .collect::<Vec<_>>()
    };
    let good = "[[draw.accounts]]\nname = \"Home\"\nkey = { secret = \"home\" }\n";
    let found = problems(&format!(
        "{good}\n[[draw.accounts]]\nkey = \"sk-plain\"\nweight = 12\nnote = \"x\"\n"
    ));
    assert_eq!(
        found,
        [
            ("draw.accounts[1].name".to_string(), Code::MissingField),
            ("draw.accounts[1].key".to_string(), Code::WrongType),
            ("draw.accounts[1].weight".to_string(), Code::OutOfRange),
            ("draw.accounts[1].note".to_string(), Code::UnknownField),
        ],
        "照在文件里的先后（少写的指在那一项的表头）；第几项从 0 数；密钥写成明文是类型不对"
    );
    assert_eq!(
        problems("[draw]\naccounts = [{ name = \"A\", key = { secret = \"a\" } }, 3]\n"),
        [("draw.accounts".to_string(), Code::WrongType)],
        "行内的不是表"
    );
    assert_eq!(
        problems("[draw]\naccounts = \"x\"\n"),
        [("draw.accounts".to_string(), Code::WrongType)]
    );
    let many: String = (0..101)
        .map(|n| format!("[[draw.accounts]]\nname = \"A{n}\"\nkey = {{ secret = \"a\" }}\n"))
        .collect();
    assert_eq!(
        problems(&many),
        [("draw.accounts".to_string(), Code::OutOfRange)],
        "最多 100 项"
    );
    let hundred: String = (0..100)
        .map(|n| format!("[[draw.accounts]]\nname = \"A{n}\"\nkey = {{ secret = \"a\" }}\n"))
        .collect();
    let parsed = parse(&items, Layer::System, &hundred).expect("读得懂");
    assert!(parsed.problems.is_empty(), "正好 100 项收");
}

#[test]
fn field_problems_are_told_by_the_field() {
    let items = draw();
    assert_eq!(
        crate::key::described_by(&items, "draw.accounts[1].weight").map(|item| item.kind),
        Some(Kind::Int { min: 1, max: 9 }),
        "问题报到字段时照字段的类型说"
    );
    assert!(crate::key::described_by(&items, "draw.accounts[1].nothing").is_none());
    assert!(
        crate::key::item_of(&items, "draw.accounts[1].weight").is_none(),
        "改配置、查键不认字段：字段不能单改"
    );
    assert_eq!(
        crate::value_problems(
            items[0].kind,
            &Value::List(vec![record(&[("name", text("A"))])].into())
        ),
        [("[0].key".to_string(), Code::MissingField)]
    );
}

/// 字段里的密钥引用也算（施工 PC-2）：没设的报 `unknown_secret`，`secret.list` 的 `used_by` 照「第几项的哪个字段」写。
#[test]
fn secrets_inside_records_are_seen() {
    let items = draw();
    let text = "[[draw.accounts]]\nname = \"Home\"\nkey = { secret = \"home\" }\n\n[[draw.accounts]]\nname = \"Work\"\nkey = { secret = \"gone\" }\n";
    let parsed = parse(&items, Layer::System, text).expect("读得懂");
    let missing = crate::secret::missing(
        &items,
        &parsed,
        Layer::System,
        &|name| name == "home",
        &|_| true,
    );
    let found: Vec<(Option<String>, Code, Option<String>)> = missing
        .iter()
        .map(|problem| (problem.key.clone(), problem.code, problem.name.clone()))
        .collect();
    assert_eq!(
        found,
        [(
            Some("draw.accounts[1].key".to_string()),
            Code::UnknownSecret,
            Some("gone".to_string())
        )]
    );
    let mut values = crate::Values::default();
    values.set(
        "draw.accounts",
        parsed.entries["draw.accounts"].value.clone(),
    );
    assert_eq!(
        crate::secret::used(&values),
        [
            ("home".to_string(), "draw.accounts[0].key".to_string()),
            ("gone".to_string(), "draw.accounts[1].key".to_string()),
        ]
    );
}
