//! 列表里每一项是一组字段（施工 PC-2，设计 53 第二节）：`element = "record"`，字段一张表一个，`title` 挑一个字段当标题。

use super::*;

const ACCOUNTS: &str = r#"[settings.accounts]
type = "list"
element = "record"
title = "name"
name = { zh = "账号", en = "Accounts" }

[settings.accounts.fields.name]
type = "text"
name = { zh = "备注" }

[settings.accounts.fields.base_url]
type = "url"
default = "https://api.example.com/v1"
name = { zh = "接口地址" }

[settings.accounts.fields.key]
type = "secret"
name = { zh = "Key" }

[settings.accounts.fields.mode]
type = "option"
choices = [{ value = "fast", name = { zh = "快" } }, { value = "good" }]
hidden = true
name = { zh = "模式" }
"#;

#[test]
fn a_record_list_reads_its_fields_and_title() {
    let accounts = setting(ACCOUNTS);
    let SettingKind::List(element) = &accounts.kind else {
        panic!("{:?}", accounts.kind);
    };
    let SettingKind::Record { fields, title } = element.as_ref() else {
        panic!("{element:?}");
    };
    assert_eq!(title.as_deref(), Some("name"));
    let names: Vec<&str> = fields.iter().map(|field| field.name.as_str()).collect();
    assert_eq!(names, ["name", "base_url", "key", "mode"], "照写的先后");
    assert_eq!(fields[0].kind, SettingKind::Text { max: 200 });
    assert_eq!(
        fields[1].default,
        Some(Value::Text("https://api.example.com/v1".into()))
    );
    assert_eq!(fields[2].kind, SettingKind::Secret);
    assert!(fields[3].hidden);
    assert_eq!(fields[0].label.get("zh").map(String::as_str), Some("备注"));
    let item = &items("draw", "draw", std::slice::from_ref(&accounts))[0];
    assert_eq!(item.key, "draw.accounts");
    let Kind::List(Kind::Record(record)) = item.kind else {
        panic!("{:?}", item.kind);
    };
    assert_eq!(record.title, Some("name"));
    let keys: Vec<&str> = record.fields.iter().map(|field| field.key).collect();
    assert_eq!(
        keys,
        ["name", "base_url", "key", "mode"],
        "字段的键是字段名"
    );
    assert_eq!(record.fields[3].kind, Kind::Option(&["fast", "good"]));
    assert!(record.fields[3].ui.hidden);
    assert_eq!(item.ui.control, Control::List);
}

#[test]
fn every_wrong_record_says_what() {
    let base =
        "[settings.accounts]\ntype = \"list\"\nelement = \"record\"\nname = { en = \"A\" }\n";
    let field = |body: &str| {
        format!("{base}\n[settings.accounts.fields.x]\n{body}\nname = {{ en = \"X\" }}\n")
    };
    for (text, code) in [
        // 一个字段都没有。
        (base.to_string(), Code::MissingKey),
        // 字段不能再是列表、record。
        (field("type = \"list\"\nelement = \"text\""), Code::BadType),
        (field("type = \"record\""), Code::BadType),
        // 字段里不写 layers、applies（跟着整项）。
        (field("type = \"text\"\nlayers = [\"system\"]"), Code::UnknownKey),
        (field("type = \"text\"\napplies = \"now\""), Code::UnknownKey),
        // 字段名不能叫 fields，写法同项的名字。
        (
            format!("{base}\n[settings.accounts.fields.fields]\ntype = \"text\"\nname = {{ en = \"F\" }}\n"),
            Code::BadSettingName,
        ),
        (
            format!("{base}\n[settings.accounts.fields.Bad]\ntype = \"text\"\nname = {{ en = \"F\" }}\n"),
            Code::BadSettingName,
        ),
        // 字段的默认值照字段的类型查；密钥不能有默认值。
        (field("type = \"int\"\ndefault = \"x\""), Code::BadDefault),
        (field("type = \"secret\"\ndefault = \"sk\""), Code::BadDefault),
        // title 要是 text、name、option 字段之一。
        (
            field("type = \"int\"").replacen(
                "name = { en = \"A\" }\n",
                "name = { en = \"A\" }\ntitle = \"x\"\n",
                1,
            ),
            Code::BadTitle,
        ),
        (
            base.replacen("name = { en = \"A\" }\n", "name = { en = \"A\" }\ntitle = \"nothing\"\n", 1)
                + "\n[settings.accounts.fields.x]\ntype = \"text\"\nname = { en = \"X\" }\n",
            Code::BadTitle,
        ),
        // 整个 record 列表不能写默认值（项太多、密钥都放不进去）。
        (
            base.replacen("name = { en = \"A\" }\n", "name = { en = \"A\" }\ndefault = []\n", 1)
                + "\n[settings.accounts.fields.x]\ntype = \"text\"\nname = { en = \"X\" }\n",
            Code::BadDefault,
        ),
        // 不是 record 的列表不能写 fields、title。
        (
            "[settings.tags]\ntype = \"list\"\nelement = \"text\"\ntitle = \"x\"\nname = { en = \"T\" }\n".to_string(),
            Code::UnknownKey,
        ),
    ] {
        assert_eq!(wrong(&text), code, "{text}");
    }
}
