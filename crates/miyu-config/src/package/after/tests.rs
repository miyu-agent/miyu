//! `[after]`（施工 HK-2，设计 65 第四节第 2 条）：写法同 `[guard]`，两样都没写、写错了访问类别、没声明 `events.read` 的报
//! `bad_after`；没有时限这一格；不是扩展包的不能写。

use crate::package::{Code, read};

/// 一份扩展包的清单，能力是 `capabilities`，再接上 `rest`。
fn extension(capabilities: &str, rest: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Audit\" }}\n\n[command]\nname = \"audit\"\nprogram = \"audit\"\nabout = {{ en = \"Audit\" }}\n\n[process]\ncapabilities = [{capabilities}]\n\n{rest}"
    )
}

const READING: &str = "\"events.read\"";

#[test]
fn after_reads_tools_and_access() {
    let manifest = read(&extension(
        READING,
        "[after]\ntools = [\"shell\", \"write\", \"shell\"]\naccess = [\"network\"]\n",
    ))
    .unwrap();
    let after = manifest.after.expect("写了 [after]");
    assert_eq!(after.tools, ["shell", "write"], "照写的先后，去掉重复");
    assert_eq!(after.access, ["network"]);
    assert!(after.covers("write", Some("write")), "工具名对上");
    assert!(after.covers("web_fetch", Some("network")), "访问类别对上");
    assert!(after.covers("shell", None), "不认识访问类别的照工具名");
    assert!(!after.covers("read", Some("read")), "两样都对不上的不发");
    assert!(!after.covers("nope", None));
    let none = read(&extension(READING, "")).unwrap();
    assert_eq!(none.after, None, "没写的没有");
}

#[test]
fn wrong_afters_are_refused() {
    let wrong = |text: String| read(&text).expect_err("应该读不成").code;
    assert_eq!(
        wrong(extension(READING, "[after]\n")),
        Code::BadAfter,
        "两样都没写：不许什么都收"
    );
    assert_eq!(
        wrong(extension(READING, "[after]\ntools = []\naccess = []\n")),
        Code::BadAfter
    );
    assert_eq!(
        wrong(extension(
            "\"tool.guard\"",
            "[after]\ntools = [\"shell\"]\n"
        )),
        Code::BadAfter,
        "没声明 events.read"
    );
    assert_eq!(
        wrong(extension(READING, "[after]\naccess = [\"delete\"]\n")),
        Code::BadAfter,
        "不认识的访问类别"
    );
    assert_eq!(
        wrong(extension(READING, "[after]\ntools = [\"two words\"]\n")),
        Code::BadAfter,
        "不是工具名"
    );
    assert_eq!(
        wrong(extension(
            READING,
            "[after]\ntools = [\"shell\"]\ntimeout_ms = 100\n"
        )),
        Code::UnknownKey,
        "通知不等回答，没有时限"
    );
    assert_eq!(
        wrong(extension(READING, "[[after]]\ntools = [\"shell\"]\n")),
        Code::NotATable,
        "一张表"
    );
    let ui = "[package]\nprotocol = [1, 1]\nname = { en = \"X\" }\n\n[ui]\n\n[after]\ntools = [\"shell\"]\n";
    assert_eq!(wrong(ui.to_string()), Code::WrongKind, "只有扩展包能写");
}

#[test]
fn guard_and_after_sit_side_by_side() {
    let manifest = read(&extension(
        "\"tool.guard\", \"events.read\"",
        "[guard]\ntools = [\"shell\"]\n\n[after]\naccess = [\"execute\"]\n",
    ))
    .unwrap();
    assert!(manifest.guard.is_some());
    assert_eq!(manifest.after.unwrap().access, ["execute"]);
}
