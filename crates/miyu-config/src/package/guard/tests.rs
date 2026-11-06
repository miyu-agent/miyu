//! `[guard]`（施工 HK-1，设计 65 第三节第 2 条）：读成工具名、访问类别、时限；两样都没写、写错了访问类别、时限出了范围、
//! 没声明 `tool.guard` 能力的报 `bad_guard`；不是扩展包的不能写。

use super::{ACCESS, TIMEOUT_DEFAULT, TIMEOUT_MAX, TIMEOUT_MIN};
use crate::package::{Code, read};

/// 一份扩展包的清单，能力是 `capabilities`，再接上 `rest`。
fn extension(capabilities: &str, rest: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Policy\" }}\n\n[command]\nname = \"policy\"\nprogram = \"policy\"\nabout = {{ en = \"Policy\" }}\n\n[process]\ncapabilities = [{capabilities}]\n\n{rest}"
    )
}

const GUARDING: &str = "\"tool.guard\"";

#[test]
fn a_guard_reads_tools_access_and_timeout() {
    let manifest = read(&extension(
        GUARDING,
        "[guard]\ntools = [\"shell\", \"write\", \"shell\"]\naccess = [\"execute\"]\ntimeout_ms = 250\n",
    ))
    .unwrap();
    let guard = manifest.guard.expect("写了 [guard]");
    assert_eq!(guard.tools, ["shell", "write"], "照写的先后，去掉重复");
    assert_eq!(guard.access, ["execute"]);
    assert_eq!(guard.timeout_ms, 250);
    assert!(guard.covers("write", "write"), "工具名对上");
    assert!(guard.covers("mcp__x__run", "execute"), "访问类别对上");
    assert!(!guard.covers("read", "read"), "两样都对不上的不问");
}

#[test]
fn either_list_alone_is_enough_and_the_timeout_defaults() {
    let tools = read(&extension(GUARDING, "[guard]\ntools = [\"edit\"]\n")).unwrap();
    let guard = tools.guard.unwrap();
    assert!(guard.access.is_empty());
    assert_eq!(guard.timeout_ms, TIMEOUT_DEFAULT);
    let access = read(&extension(GUARDING, "[guard]\naccess = [\"network\"]\n")).unwrap();
    assert!(access.guard.unwrap().tools.is_empty());
    let none = read(&extension(GUARDING, "")).unwrap();
    assert_eq!(none.guard, None, "没写的没有");
}

#[test]
fn every_access_class_is_taken() {
    for access in ACCESS {
        let text = extension(GUARDING, &format!("[guard]\naccess = [\"{access}\"]\n"));
        assert!(read(&text).is_ok(), "{access}");
    }
}

#[test]
fn the_timeout_stays_in_range() {
    for ms in [TIMEOUT_MIN, TIMEOUT_MAX] {
        let text = extension(
            GUARDING,
            &format!("[guard]\ntools = [\"shell\"]\ntimeout_ms = {ms}\n"),
        );
        assert_eq!(read(&text).unwrap().guard.unwrap().timeout_ms, ms);
    }
    for ms in [
        (TIMEOUT_MIN - 1).to_string(),
        (TIMEOUT_MAX + 1).to_string(),
        "-5".to_string(),
        "1.5".to_string(),
        "\"1000\"".to_string(),
    ] {
        let text = extension(
            GUARDING,
            &format!("[guard]\ntools = [\"shell\"]\ntimeout_ms = {ms}\n"),
        );
        assert_eq!(read(&text).unwrap_err().code, Code::BadGuard, "{ms}");
    }
}

#[test]
fn wrong_guards_are_refused() {
    let wrong = |text: String| read(&text).expect_err("应该读不成").code;
    assert_eq!(
        wrong(extension(GUARDING, "[guard]\n")),
        Code::BadGuard,
        "两样都没写：不许什么都问"
    );
    assert_eq!(
        wrong(extension(GUARDING, "[guard]\ntools = []\naccess = []\n")),
        Code::BadGuard,
        "两样都是空的"
    );
    assert_eq!(
        wrong(extension("", "[guard]\ntools = [\"shell\"]\n")),
        Code::BadGuard,
        "没声明 tool.guard"
    );
    assert_eq!(
        wrong(extension(GUARDING, "[guard]\naccess = [\"delete\"]\n")),
        Code::BadGuard,
        "不认识的访问类别"
    );
    assert_eq!(
        wrong(extension(GUARDING, "[guard]\ntools = [\"two words\"]\n")),
        Code::BadGuard,
        "不是工具名"
    );
    assert_eq!(
        wrong(extension(GUARDING, "[guard]\ntools = [\"\"]\n")),
        Code::BadGuard
    );
    assert_eq!(
        wrong(extension(GUARDING, "[guard]\ntools = \"shell\"\n")),
        Code::BadGuard,
        "要写成数组"
    );
    assert_eq!(
        wrong(extension(
            GUARDING,
            "[guard]\ntools = [\"shell\"]\nall = true\n"
        )),
        Code::UnknownKey
    );
    assert_eq!(
        wrong(extension(GUARDING, "[[guard]]\ntools = [\"shell\"]\n")),
        Code::NotATable,
        "一张表"
    );
    let ui = "[package]\nprotocol = [1, 1]\nname = { en = \"X\" }\n\n[ui]\n\n[guard]\ntools = [\"shell\"]\n";
    assert_eq!(wrong(ui.to_string()), Code::WrongKind, "只有扩展包能写");
}
