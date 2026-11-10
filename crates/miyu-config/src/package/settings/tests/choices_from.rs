//! 选项由包在运行时给（施工 SG-2）：`type = "text"` 的项可以写 `choices_from = "<方法名>"`，读成 [`Setting::choices_from`]；
//! 没写的没有。写在几行字、列表、一组字段的字段上，方法名写法不对、不是字的报 `bad_choices_from`；写在别的类型上的照旧是
//! 不认识的键。值照旧是普通文字：默认值不照列表查。

use super::*;

/// 一个选麦克风的文字项：`rest` 接在后面。
fn mic(rest: &str) -> String {
    format!("[settings.mic]\ntype = \"text\"\nname = {{ en = \"Microphone\" }}\n{rest}\n")
}

#[test]
fn a_text_reads_where_its_choices_come_from() {
    let named = setting(&mic("choices_from = \"devices.list\"\ndefault = \"hw:9\""));
    assert_eq!(named.choices_from.as_deref(), Some("devices.list"));
    assert_eq!(named.kind, SettingKind::Text { max: 200 }, "照旧是文字");
    assert_eq!(
        named.default,
        Some(Value::Text("hw:9".to_string().into())),
        "默认值不照列表查"
    );
    assert_eq!(setting(&mic("")).choices_from, None, "没写的没有");
    let grouped = setting(
        "[settings.input.mic]\ntype = \"text\"\nname = { en = \"Mic\" }\nchoices_from = \"mics\"\n",
    );
    assert_eq!(grouped.name, "input.mic");
    assert_eq!(
        grouped.choices_from.as_deref(),
        Some("mics"),
        "组里的也能写"
    );
}

#[test]
fn the_longest_method_name_is_64() {
    let name = "a".repeat(64);
    let read = setting(&mic(&format!("choices_from = \"{name}\"")));
    assert_eq!(read.choices_from, Some(name));
}

#[test]
fn a_wrong_choices_from_is_a_problem() {
    let long = format!("choices_from = \"{}\"", "a".repeat(65));
    let cases = [
        ("choices_from = \"\"", "空的"),
        ("choices_from = \"list devices\"", "带空白"),
        ("choices_from = \"Devices\"", "大写开头"),
        ("choices_from = \"9devices\"", "数字开头"),
        ("choices_from = \"设备\"", "不是 ASCII"),
        (long.as_str(), "超过 64 个"),
        ("choices_from = 1", "不是字"),
        ("choices_from = [\"devices\"]", "写成列表"),
        ("choices_from = { method = \"devices\" }", "写成表"),
        ("lines = true\nchoices_from = \"devices\"", "几行字"),
    ];
    for (rest, why) in cases {
        let problem = read(&web(&mic(rest))).expect_err(why);
        assert_eq!(problem.code, Code::BadChoicesFrom, "{why}：{problem:?}");
        assert_eq!(problem.detail, "settings.mic.choices_from", "{why}");
        assert!(problem.line.is_some(), "{why}：报在那一行");
    }
}

#[test]
fn only_a_plain_text_setting_takes_choices_from() {
    assert_eq!(
        wrong(
            "[settings.mics]\ntype = \"list\"\nelement = \"text\"\nname = { en = \"M\" }\nchoices_from = \"devices\"\n"
        ),
        Code::BadChoicesFrom,
        "文字的列表"
    );
    assert_eq!(
        wrong(
            "[settings.rooms]\ntype = \"list\"\nelement = \"record\"\nname = { en = \"R\" }\n\n[settings.rooms.fields.mic]\ntype = \"text\"\nname = { en = \"M\" }\nchoices_from = \"devices\"\n"
        ),
        Code::BadChoicesFrom,
        "一组字段里的字段"
    );
    for kind in ["bool", "int", "name", "url", "secret", "model"] {
        assert_eq!(
            wrong(&format!(
                "[settings.x]\ntype = \"{kind}\"\nname = {{ en = \"X\" }}\nchoices_from = \"devices\"\n"
            )),
            Code::UnknownKey,
            "{kind}：别的类型不认这一格"
        );
    }
    assert_eq!(
        wrong(
            "[settings.x]\ntype = \"option\"\nchoices = [\"a\", \"b\"]\nname = { en = \"X\" }\nchoices_from = \"devices\"\n"
        ),
        Code::UnknownKey,
        "写死了选项的不认"
    );
}
