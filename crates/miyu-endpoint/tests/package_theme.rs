//! 主题包（施工 F-16，`docs/blueprint/packages.md`「主题包」）：真核心走一遍，同吉祥物包。清单只有 `[package]`、`[theme]`，不说
//! 协议、不带吉祥物；`package.list` 带 `theme`，`package.file` 只给主题文件那一份，`miyu check` 查主题文件在、不超过 256 KiB、
//! 是 JSON 的对象；没有开关。

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::support::*;
use miyu_session::testkit::Script;

/// 主题包的清单。
const ASCII: &str = "[package]\nname = { zh = \"纯字符\", en = \"ASCII\" }\n\n[theme]\nfile = \"look/theme.json\"\n";

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// `miyu check` 报的主题那几条：代码和说法。
fn theme_problems(reply: &Value) -> Vec<(String, String)> {
    reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|problem| {
            problem["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("theme_"))
        })
        .map(|problem| {
            (
                problem["code"].as_str().unwrap_or_default().to_string(),
                problem["message"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_theme_is_listed_and_its_file_is_read() {
    let home = Home::new();
    home.write("home/alice/packages/ascii/package.toml", ASCII);
    home.write(
        "home/alice/packages/ascii/look/theme.json",
        "{\"frame\": \"round\"}",
    );
    home.write("home/alice/packages/ascii/notes.txt", "not the theme");
    let mut client = connected(&home).await;
    let listed = client.call("l", "package.list", json!({})).await;
    let ascii = listed["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .find(|item| item["package"] == "ascii")
        .cloned()
        .unwrap_or_else(|| panic!("列出来了：{listed}"));
    assert_eq!(ascii["kind"], "theme");
    assert_eq!(ascii["name"], "纯字符");
    assert_eq!(ascii["theme"], json!({"file": "look/theme.json"}));
    assert!(ascii.get("protocol").is_none(), "不说协议：{ascii}");

    let model = client
        .call(
            "f",
            "package.file",
            json!({"package": "ascii", "path": "look/theme.json"}),
        )
        .await;
    let data = STANDARD
        .decode(
            model["result"]["data"]
                .as_str()
                .unwrap_or_else(|| panic!("{model}")),
        )
        .expect("是 base64");
    assert_eq!(data, b"{\"frame\": \"round\"}");
    for path in ["notes.txt", "", "art/../look/theme.json"] {
        let other = client
            .call(
                "o",
                "package.file",
                json!({"package": "ascii", "path": path}),
            )
            .await;
        assert!(
            other.get("error").is_some(),
            "只给主题文件那一份：{path:?} {other}"
        );
    }
    let switched = client
        .call("e", "package.disable", json!({"package": "ascii"}))
        .await;
    assert_eq!(
        switched["error"]["data"]["reason"], "not_switchable",
        "{switched}"
    );
}

#[tokio::test]
async fn check_says_when_the_theme_is_missing_too_big_or_not_an_object() {
    let home = Home::new();
    home.write("home/alice/packages/ascii/package.toml", ASCII);
    let mut client = connected(&home).await;
    let model = "home/alice/packages/ascii/look/theme.json";
    let reply = client.call("c1", "check", json!({})).await;
    assert_eq!(
        theme_problems(&reply),
        [(
            "theme_missing".to_string(),
            "包目录里没有 look/theme.json".to_string()
        )],
        "{reply}"
    );
    home.write(model, "[1, 2]");
    let reply = client.call("c2", "check", json!({})).await;
    assert_eq!(
        theme_problems(&reply),
        [(
            "theme_not_json".to_string(),
            "look/theme.json 不是 JSON 对象".to_string()
        )]
    );
    home.write(model, &format!("{{\"x\": \"{}\"}}", "a".repeat(300 * 1024)));
    let reply = client.call("c3", "check", json!({})).await;
    assert_eq!(
        theme_problems(&reply),
        [(
            "theme_too_big".to_string(),
            "look/theme.json 超过 256 KiB".to_string()
        )]
    );
    home.write(model, "{\"frame\": \"round\"}");
    let reply = client.call("c4", "check", json!({})).await;
    assert!(theme_problems(&reply).is_empty(), "{reply}");
}

#[tokio::test]
async fn a_theme_takes_no_protocol_and_no_other_tables() {
    let home = Home::new();
    home.write(
        "home/alice/packages/a/package.toml",
        &ASCII.replace("[package]\n", "[package]\nprotocol = [1, 1]\n"),
    );
    home.write(
        "home/alice/packages/b/package.toml",
        &format!("{ASCII}\n[command]\nname = \"b\"\nprogram = \"b\"\n"),
    );
    home.write(
        "home/alice/packages/c/package.toml",
        &ASCII.replace("look/theme.json", "../theme.json"),
    );
    home.write(
        "home/alice/packages/d/package.toml",
        "[package]\nname = { en = \"D\" }\n\n[theme]\n",
    );
    // 主题包不带吉祥物（施工 F-16）。
    home.write(
        "home/alice/packages/e/package.toml",
        &format!("{ASCII}\n[mascot]\nmodel = \"m.json\"\n"),
    );
    let mut client = connected(&home).await;
    let reply = client.call("c", "check", json!({})).await;
    let codes: Vec<(String, String)> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|problem| problem["kind"] == "package")
        .map(|problem| {
            (
                problem["file"].as_str().unwrap_or_default().to_string(),
                problem["code"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        codes,
        [
            (
                "home/alice/packages/a/package.toml".to_string(),
                "wrong_kind".to_string()
            ),
            (
                "home/alice/packages/b/package.toml".to_string(),
                "wrong_kind".to_string()
            ),
            (
                "home/alice/packages/c/package.toml".to_string(),
                "bad_theme_file".to_string()
            ),
            (
                "home/alice/packages/d/package.toml".to_string(),
                "missing_key".to_string()
            ),
            (
                "home/alice/packages/e/package.toml".to_string(),
                "wrong_kind".to_string()
            ),
        ],
        "{reply}"
    );
}
