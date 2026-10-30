//! 配置项的词汇（施工 PC-1，设计 53 第一、三节）：选项带显示名、说明；多行文字；小数；滑块。

use super::*;

const BACKGROUND: &str = r#"[settings.background]
type = "option"
choices = [
  { value = "auto", name = { zh = "自动", en = "Auto" } },
  { value = "transparent", name = { zh = "透明", en = "Transparent" }, description = { zh = "要配 png、webp" } },
  { value = "opaque" },
]
default = "auto"
name = { en = "Background" }
"#;

#[test]
fn choices_can_carry_a_name_and_a_description() {
    let background = setting(BACKGROUND);
    let SettingKind::Option(choices) = &background.kind else {
        panic!("{:?}", background.kind);
    };
    let values: Vec<&str> = choices.iter().map(|choice| choice.value.as_str()).collect();
    assert_eq!(values, ["auto", "transparent", "opaque"]);
    assert_eq!(choices[0].name.get("zh").map(String::as_str), Some("自动"));
    assert_eq!(
        choices[1].description.get("zh").map(String::as_str),
        Some("要配 png、webp")
    );
    assert!(
        choices[2].name.is_empty() && choices[2].description.is_empty(),
        "没写的是空的"
    );
    assert_eq!(
        background.default,
        Some(Value::Text("auto".into())),
        "存的照旧是值"
    );
    let item = &items("web", "web", std::slice::from_ref(&background))[0];
    assert_eq!(item.kind, Kind::Option(&["auto", "transparent", "opaque"]));
    assert_eq!(item.ui.control, Control::Select);
}

#[test]
fn every_wrong_choice_says_what() {
    let choices = |list: &str| {
        format!("[settings.x]\ntype = \"option\"\nchoices = {list}\nname = {{ en = \"X\" }}\n")
    };
    for list in [
        // 字和表混着写。
        r#"["a", { value = "b" }]"#,
        // 少了 value。
        r#"[{ value = "a" }, { name = { en = "B" } }]"#,
        // value 不是字。
        r#"[{ value = "a" }, { value = 2 }]"#,
        // 不认识的格。
        r#"[{ value = "a" }, { value = "b", note = "x" }]"#,
        // 重了。
        r#"[{ value = "a" }, { value = "a" }]"#,
        // 只有一个。
        r#"[{ value = "a" }]"#,
    ] {
        assert_eq!(wrong(&choices(list)), Code::BadChoices, "{list}");
    }
    assert_eq!(
        wrong(&choices(r#"[{ value = "a", name = 3 }, { value = "b" }]"#)),
        Code::NotPhrases,
        "名字照项的名字那样查"
    );
}

#[test]
fn lines_number_and_sliders_read() {
    let prompt =
        setting("[settings.prompt]\ntype = \"text\"\nlines = true\nname = { en = \"Prompt\" }\n");
    assert_eq!(
        prompt.kind,
        SettingKind::Lines { max: 2000 },
        "不写 max 最多 2000"
    );
    let short = setting(
        "[settings.prompt]\ntype = \"text\"\nlines = true\nmax = 50\nname = { en = \"P\" }\n",
    );
    assert_eq!(short.kind, SettingKind::Lines { max: 50 });
    let plain =
        setting("[settings.prompt]\ntype = \"text\"\nlines = false\nname = { en = \"P\" }\n");
    assert_eq!(plain.kind, SettingKind::Text { max: 200 });
    let temperature = setting(
        "[settings.temperature]\ntype = \"number\"\nmin = 0\nmax = 2\nstep = 0.1\nslider = true\ndefault = 0.7\nname = { en = \"T\" }\n",
    );
    assert_eq!(temperature.kind, SettingKind::Number { min: 0, max: 2 });
    assert_eq!(temperature.default, Some(Value::Float(Number::new(0.7))));
    let top =
        setting("[settings.top]\ntype = \"number\"\nmin = 0\nmax = 1\nname = { en = \"P\" }\n");
    let count = setting(
        "[settings.count]\ntype = \"int\"\nmin = 1\nmax = 8\nslider = true\nname = { en = \"N\" }\n",
    );
    let wide = setting(
        "[settings.wide]\ntype = \"number\"\nmin = 0\nmax = 50\nslider = true\nname = { en = \"W\" }\n",
    );
    let all = [prompt, temperature, top, count, wide];
    let items = items("web", "web", &all);
    let shapes: Vec<(Kind, Control)> = items
        .iter()
        .map(|item| (item.kind, item.ui.control))
        .collect();
    assert_eq!(
        shapes,
        [
            (Kind::Lines { max: 2000 }, Control::TextArea),
            (
                Kind::Float { min: 0, max: 2 },
                Control::Slider {
                    step: Number::new(0.1)
                }
            ),
            (Kind::Float { min: 0, max: 1 }, Control::Number),
            (
                Kind::Int { min: 1, max: 8 },
                Control::Slider {
                    step: Number::new(1.0)
                }
            ),
            (
                Kind::Float { min: 0, max: 50 },
                Control::Slider {
                    step: Number::new(0.5)
                }
            ),
        ],
        "不写 step 的滑块：整数是 1，小数是范围的百分之一"
    );
}

#[test]
fn every_wrong_number_lines_or_slider_says_what() {
    let x = |body: &str| format!("[settings.x]\n{body}\nname = {{ en = \"X\" }}\n");
    for (body, code) in [
        // 小数要写范围，范围只收整数（精度照 step）。
        ("type = \"number\"\nmax = 2", Code::BadRange),
        ("type = \"number\"\nmin = 0", Code::BadRange),
        ("type = \"number\"\nmin = 0.5\nmax = 2", Code::BadRange),
        ("type = \"number\"\nmin = 3\nmax = 2", Code::BadRange),
        (
            "type = \"number\"\nmin = 0\nmax = 2\nstep = 0",
            Code::BadRange,
        ),
        (
            "type = \"number\"\nmin = 0\nmax = 2\ndefault = 3.5",
            Code::BadDefault,
        ),
        // 整数的 step 也是整数；滑块要两头都写。
        (
            "type = \"int\"\nmin = 0\nmax = 9\nstep = 0.5",
            Code::BadRange,
        ),
        ("type = \"int\"\nmin = 0\nslider = true", Code::BadRange),
        (
            "type = \"int\"\nslider = \"yes\"\nmin = 0\nmax = 9",
            Code::NotBool,
        ),
        ("type = \"text\"\nlines = 1", Code::NotBool),
        // 别的类型没有这几格。
        ("type = \"bool\"\nslider = true", Code::UnknownKey),
        ("type = \"name\"\nlines = true", Code::UnknownKey),
        ("type = \"text\"\nstep = 1", Code::UnknownKey),
    ] {
        assert_eq!(wrong(&x(body)), code, "{body}");
    }
}
