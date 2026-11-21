//! 包里的工具说明有不是各家都收的参数写法（施工 FX-10）：真核心照装，装包的回应带一条警告 `tool_not_portable`，`check` 也报
//! 这一条（警告，不是错误）；写对了的什么都不报。接入通讯平台的一件工具 2026-11-20 把 `enum` 写在整数上，用 Gemini 时整个请求
//! 被拒。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 在工作目录里放一个只带一份工具说明的包 `rank`：`days` 的类型是 `kind`，`enum` 写 `values`。交回文件夹的路径。
fn rank(home: &Home, kind: &str, values: &str) -> String {
    let folder = home.work.join("rank");
    std::fs::create_dir_all(folder.join("data/qq/tools")).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nversion = \"1.0.0\"\nname = { en = \"Rank\" }\n",
    )
    .expect("写得进");
    std::fs::write(
        folder.join("data/qq/tools/ranking.json"),
        format!(
            r#"{{"description":"Rank.","parameters":{{"type":"object","properties":{{"days":{{"type":"{kind}","enum":{values}}}}}}}}}"#
        ),
    )
    .expect("写得进");
    folder.to_string_lossy().into_owned()
}

/// `check` 报的这个包的工具说明那几条。
fn unportable(reply: &Value) -> Vec<Value> {
    reply["result"]["problems"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|one| one["code"] == "tool_not_portable")
        .cloned()
        .collect()
}

#[tokio::test]
async fn an_enum_on_an_integer_installs_with_a_warning() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let installed = client
        .call(
            "i1",
            "package.install",
            json!({"path": rank(&home, "integer", "[1,7,30]")}),
        )
        .await;
    assert_eq!(installed["result"]["package"], "rank", "照装：{installed}");
    let warnings = &installed["result"]["warnings"];
    assert_eq!(warnings.as_array().map(Vec::len), Some(1), "{installed}");
    assert_eq!(warnings[0]["code"], "tool_not_portable");
    assert_eq!(warnings[0]["level"], "warning");
    assert_eq!(
        warnings[0]["message"],
        "工具说明 data/qq/tools/ranking.json /properties/days 在非字符串类型上使用了 enum，部分模型供应商会拒绝整个请求"
    );
    let checked = client.call("k1", "check", json!({})).await;
    let said = unportable(&checked);
    assert_eq!(said.len(), 1, "{checked}");
    assert_eq!(said[0]["level"], "warning");
    assert_eq!(said[0]["kind"], "package");
    assert_eq!(said[0]["message"], warnings[0]["message"]);
}

#[tokio::test]
async fn an_enum_on_strings_says_nothing() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let installed = client
        .call(
            "i1",
            "package.install",
            json!({"path": rank(&home, "string", r#"["1","7"]"#)}),
        )
        .await;
    assert_eq!(installed["result"]["package"], "rank", "{installed}");
    assert!(installed["result"].get("warnings").is_none(), "{installed}");
    let checked = client.call("k1", "check", json!({})).await;
    assert!(unportable(&checked).is_empty(), "{checked}");
}
