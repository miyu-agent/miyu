//! `[[widget]]`（施工 X-15 上）：每一格读成什么、没写的照什么；每一种写错报 `bad_widget`、报在那一段；要声明 `widgets`
//! 能力；只有扩展包能写；最多 16 块；整页的命令名和这个包的斜杠命令不重。

use super::super::{Code, WidgetPlace, read};

/// 一个扩展：声明了 `widgets` 能力、三块。
const EXTENSION: &str = r#"[package]
protocol = [1, 1]
name = { en = "Weather" }

[command]
name = "weather"
program = "miyu-weather"
about = { en = "Weather" }

[process]
capabilities = ["widgets", "commands"]

[[widget]]
id = "today"
place = "sidebar"
title = { en = "Weather", zh = "天气" }
scope = "session"

[[widget]]
id = "temp"
place = "status"

[[widget]]
id = "week"
place = "page"
title = { en = "This week" }
command = "week"
"#;

/// 换掉 `EXTENSION` 里第一块的写法。
fn with_first(widget: &str) -> String {
    let start = EXTENSION.find("[[widget]]").expect("有");
    let end = EXTENSION.find("[[widget]]\nid = \"temp\"").expect("有");
    format!("{}{widget}\n{}", &EXTENSION[..start], &EXTENSION[end..])
}

#[test]
fn an_extension_declares_its_widgets_with_every_field() {
    let manifest = read(EXTENSION).expect("合写法");
    assert_eq!(manifest.widgets.len(), 3);
    let today = &manifest.widgets[0];
    assert_eq!(today.id, "today");
    assert_eq!(today.place, WidgetPlace::Sidebar);
    assert_eq!(
        today
            .title
            .as_ref()
            .and_then(|title| title.get("zh"))
            .map(String::as_str),
        Some("天气")
    );
    assert!(today.per_session);
    assert_eq!(today.command, None);
    assert_eq!(today.line, Some(13));
    let temp = &manifest.widgets[1];
    assert_eq!(
        (temp.place, temp.title.is_none(), temp.per_session),
        (WidgetPlace::Status, true, false)
    );
    let week = &manifest.widgets[2];
    assert_eq!(week.place, WidgetPlace::Page);
    assert_eq!(week.command.as_deref(), Some("week"));
    assert_eq!(week.place.as_str(), "page");
}

#[test]
fn each_mistake_is_bad_widget_on_its_table() {
    let cases = [
        (
            "[[widget]]\nid = \"Today\"\nplace = \"home\"\n",
            "widget.id \"Today\"",
        ),
        ("[[widget]]\nplace = \"home\"\n", "widget.id"),
        ("[[widget]]\nid = \"today\"\n", "widget.today.place"),
        (
            "[[widget]]\nid = \"today\"\nplace = \"footer\"\n",
            "widget.today.place",
        ),
        (
            "[[widget]]\nid = \"today\"\nplace = \"sidebar\"\n",
            "widget.today.title",
        ),
        (
            "[[widget]]\nid = \"today\"\nplace = \"page\"\n",
            "widget.today.title",
        ),
        (
            "[[widget]]\nid = \"today\"\nplace = \"home\"\nscope = \"session\"\n",
            "widget.today.scope",
        ),
        (
            "[[widget]]\nid = \"today\"\nplace = \"status\"\nscope = \"session\"\n",
            "widget.today.scope",
        ),
        (
            "[[widget]]\nid = \"today\"\nplace = \"page\"\ntitle = { en = \"x\" }\nscope = \"session\"\n",
            "widget.today.scope",
        ),
        (
            "[[widget]]\nid = \"today\"\nplace = \"sidebar\"\ntitle = { en = \"x\" }\nscope = \"room\"\n",
            "widget.today.scope",
        ),
        (
            "[[widget]]\nid = \"today\"\nplace = \"sidebar\"\ntitle = { en = \"x\" }\ncommand = \"today\"\n",
            "widget.today.command",
        ),
        (
            "[[widget]]\nid = \"today\"\nplace = \"page\"\ntitle = { en = \"x\" }\ncommand = \"Today\"\n",
            "widget.today.command",
        ),
    ];
    for (widget, detail) in cases {
        let problem = read(&with_first(widget)).expect_err(widget);
        assert_eq!(problem.code, Code::BadWidget, "{widget}: {problem:?}");
        assert_eq!(problem.detail, detail, "{widget}");
        assert_eq!(problem.line, Some(13), "{widget}");
    }
    // 首页、底栏可以不写标题，全局是不写 scope 的那一种。
    let home = read(&with_first(
        "[[widget]]\nid = \"today\"\nplace = \"home\"\nscope = \"global\"\n",
    ))
    .expect("首页不用标题");
    assert!(!home.widgets[0].per_session);
    // 同一个包里重了的报在后写的那一段。
    let twice =
        read(&with_first("[[widget]]\nid = \"temp\"\nplace = \"home\"\n")).expect_err("重了");
    assert_eq!(
        (twice.code, twice.detail.as_str(), twice.line),
        (Code::BadWidget, "widget.temp", Some(17))
    );
    let unknown = read(&with_first(
        "[[widget]]\nid = \"today\"\nplace = \"home\"\nsize = 3\n",
    ))
    .expect_err("不认识的键");
    assert_eq!(unknown.code, Code::UnknownKey);
}

#[test]
fn declaring_widgets_needs_the_widgets_capability() {
    let without = EXTENSION.replace(
        "capabilities = [\"widgets\", \"commands\"]",
        "capabilities = [\"commands\"]",
    );
    let problem = read(&without).expect_err("没声明能力");
    assert_eq!(
        (problem.code, problem.detail.as_str(), problem.line),
        (Code::BadWidget, "widget", Some(13))
    );
}

#[test]
fn a_page_command_may_not_repeat_a_slash_command_of_the_same_package() {
    let clash = format!("{EXTENSION}\n[[slash]]\nname = \"week\"\nsummary = {{ en = \"x\" }}\n");
    let problem = read(&clash).expect_err("撞了自己的斜杠命令");
    assert_eq!(
        (problem.code, problem.detail.as_str(), problem.line),
        (Code::BadWidget, "widget.week.command", Some(23))
    );
    let two_pages = format!(
        "{EXTENSION}\n[[widget]]\nid = \"month\"\nplace = \"page\"\ntitle = {{ en = \"x\" }}\ncommand = \"week\"\n"
    );
    let problem = read(&two_pages).expect_err("两页一个命令");
    assert_eq!(problem.detail, "widget.month.command");
}

#[test]
fn a_package_declares_at_most_sixteen_widgets() {
    let mut text = EXTENSION.to_string();
    for n in 3..16 {
        text.push_str(&format!("\n[[widget]]\nid = \"w{n}\"\nplace = \"home\"\n"));
    }
    assert_eq!(read(&text).expect("16 块正好").widgets.len(), 16);
    text.push_str("\n[[widget]]\nid = \"w16\"\nplace = \"home\"\n");
    let problem = read(&text).expect_err("17 块");
    assert_eq!(
        (problem.code, problem.detail.as_str()),
        (Code::BadWidget, "widget")
    );
}

#[test]
fn only_an_extension_may_declare_widgets() {
    let bundle = "[package]\nname = { en = \"Kit\" }\n\n[[widget]]\nid = \"x\"\nplace = \"home\"\n";
    assert_eq!(
        read(bundle).expect_err("bundle 不能写").code,
        Code::WrongKind
    );
    let not_tables = EXTENSION.replace("[[widget]]\nid = \"temp\"", "[widget]\nid = \"temp\"");
    assert!(read(&not_tables).is_err(), "写成一张表的不认");
}
