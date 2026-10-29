//! `miyu mcp login`（施工 X-42 上）：照 `mcp.list` 认服务的状态；打开网址的程序照系统挑，网址原样一个参数交过去。贴回来的一行去掉
//! 两头的空白，空的不算（施工 X-42 下）。

use serde_json::json;

use super::*;

#[test]
fn the_state_comes_from_the_listed_server() {
    let listed = json!({"servers": [
        {"name": "other", "state": "running"},
        {"name": "notes", "state": "stopped", "reason": "needs_login"},
    ]});
    assert_eq!(state(&listed, "notes"), Some("stopped"));
    assert_eq!(state(&listed, "other"), Some("running"));
    assert_eq!(state(&listed, "carried"), None, "包里带的不在 mcp.list 里");
}

#[test]
fn the_url_goes_whole_to_the_opener() {
    let url = "https://auth.example.com/authorize?a=1&b=2";
    let (program, args) = opener(url);
    assert!(!program.is_empty());
    assert_eq!(
        args.last().map(String::as_str),
        Some(url),
        "网址带着 & 也是一个参数"
    );
}

#[test]
fn a_pasted_line_is_trimmed_and_an_empty_one_is_not_taken() {
    assert_eq!(
        pasted("  http://127.0.0.1:8/callback?code=c&state=s\r\n"),
        Some("http://127.0.0.1:8/callback?code=c&state=s")
    );
    assert_eq!(pasted("   "), None);
    assert_eq!(pasted(""), None);
}

// CL-2 下（设计 47）：打不开浏览器、等到时没登录是 `miyu: ` 打头的报错，结果行不带句号。

#[test]
fn the_words_follow_the_command_line_rules() {
    use crate::diagnostic::Level;
    let (zh, en) = (Language::Chinese, Language::English);
    assert_eq!(
        zh.browser_not_opened().lines(zh, Level::Error),
        ["miyu: 无法打开浏览器，请手动打开上面的地址"]
    );
    assert_eq!(
        en.browser_not_opened().lines(en, Level::Error),
        ["miyu: cannot open a browser; open the address above manually"]
    );
    assert_eq!(
        en.login_timed_out("notes").lines(en, Level::Error),
        ["miyu: notes still not logged in after 10 minutes"]
    );
    assert_eq!(zh.logged_in("notes"), "已登录，notes 已连接");
    assert_eq!(
        en.logged_out("notes"),
        "logged out of notes, log in again to use it"
    );
}
