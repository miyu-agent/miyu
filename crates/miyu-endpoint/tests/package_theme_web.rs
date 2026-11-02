//! 外观包带网页主题（施工 TH-1，`docs/blueprint/packages.md`「外观包」）：真核心走一遍。`[theme.web] dir` 指包里的一个目录，
//! `package.list` 的 `theme` 分 `tui`、`web` 两格；`package.file` 给终端主题文件那一份和网页目录下的任何一份普通文件，路径照包目录
//! 写，出了网页目录的、链接指出去的不给；只带网页主题的包照装；网页目录不在 `miyu check` 报 `theme_missing`。

use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::support::*;
use miyu_session::testkit::Script;

/// 两样都带的外观包。
const BOTH: &str = "[package]\nname = { zh = \"夜\", en = \"Night\" }\n\n[theme.tui]\nfile = \"theme.json\"\n\n[theme.web]\ndir = \"web\"\n";

/// 只带网页主题的外观包。
const WEB_ONLY: &str = "[package]\nname = { en = \"Paper\" }\n\n[theme.web]\ndir = \"web\"\n";

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// `package.list` 里编号是 `id` 的那一项。
async fn listed(client: &mut Client, id: &str) -> Value {
    let listed = client.call("l", "package.list", json!({})).await;
    listed["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .find(|item| item["package"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("列出来了：{listed}"))
}

/// 读一块：交回解出来的字节、`size`、`eof`；出错的交回整个回应。
async fn file(
    client: &mut Client,
    id: &str,
    path: &str,
    offset: u64,
) -> Result<(Vec<u8>, u64, bool), Value> {
    let reply = client
        .call(
            "f",
            "package.file",
            json!({"package": id, "path": path, "offset": offset}),
        )
        .await;
    let result = &reply["result"];
    let Some(data) = result["data"].as_str() else {
        return Err(reply);
    };
    Ok((
        STANDARD.decode(data).expect("是 base64"),
        result["size"].as_u64().expect("有 size"),
        result["eof"].as_bool().expect("有 eof"),
    ))
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
                .is_some_and(|code| code.starts_with("theme_") || code.starts_with("bad_theme"))
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
async fn both_themes_are_listed_and_any_file_under_the_web_dir_is_read() {
    let home = Home::new();
    home.write("opt/night/package.toml", BOTH);
    home.write("opt/night/theme.json", "{\"frame\": \"round\"}");
    home.write("opt/night/notes.txt", "not a theme");
    home.write("opt/night/web/manifest.json", "{\"entry\": \"index.js\"}");
    home.write("opt/night/web/index.js", "export default 1;");
    let font: Vec<u8> = (0..=255).collect();
    std::fs::create_dir_all(home.root.packages().join("night/web/fonts")).expect("建得了");
    std::fs::write(home.root.packages().join("night/web/fonts/a.woff2"), &font).expect("写得进");
    let big = "x".repeat(600 * 1024);
    home.write("opt/night/web/big.css", &big);
    let mut client = connected(&home).await;

    let night = listed(&mut client, "night").await;
    assert_eq!(night["kind"], "theme");
    assert_eq!(
        night["theme"],
        json!({"tui": {"file": "theme.json"}, "web": {"dir": "web"}})
    );

    let (data, ..) = file(&mut client, "night", "theme.json", 0)
        .await
        .expect("给终端主题");
    assert_eq!(data, b"{\"frame\": \"round\"}");
    let (data, ..) = file(&mut client, "night", "web/index.js", 0)
        .await
        .expect("给网页目录里的");
    assert_eq!(data, b"export default 1;");
    let (data, ..) = file(&mut client, "night", "web/fonts/a.woff2", 0)
        .await
        .expect("什么类型都给");
    assert_eq!(data, font);
    // 分块照旧：一次最多 512 KiB。
    let (first, size, eof) = file(&mut client, "night", "web/big.css", 0)
        .await
        .expect("第一块");
    assert_eq!((first.len(), size, eof), (512 * 1024, 600 * 1024, false));
    let (rest, _, eof) = file(&mut client, "night", "web/big.css", 512 * 1024)
        .await
        .expect("第二块");
    assert_eq!((rest.len(), eof), (88 * 1024, true));

    for path in [
        "notes.txt",
        "",
        "web",
        "web/missing.js",
        "web/../notes.txt",
        "package.toml",
        "webx/index.js",
    ] {
        let refused = file(&mut client, "night", path, 0).await;
        assert!(
            refused.is_err(),
            "只给终端主题和网页目录下的：{path:?} {refused:?}"
        );
    }
}

#[tokio::test]
async fn a_web_only_theme_installs_and_is_listed_without_a_terminal_file() {
    let home = Home::new();
    let folder: PathBuf = home.work.join("paper");
    std::fs::create_dir_all(folder.join("web")).expect("建得了");
    std::fs::write(folder.join("package.toml"), WEB_ONLY).expect("写得进");
    std::fs::write(folder.join("web/manifest.json"), "{}").expect("写得进");
    std::fs::write(folder.join("web/index.js"), "export default 2;").expect("写得进");
    let mut client = connected(&home).await;
    let installed = client
        .call(
            "i",
            "package.install",
            json!({"path": folder.to_string_lossy()}),
        )
        .await;
    assert!(installed.get("error").is_none(), "装得上：{installed}");

    let paper = listed(&mut client, "paper").await;
    assert_eq!(paper["kind"], "theme");
    assert_eq!(paper["theme"], json!({"web": {"dir": "web"}}), "{paper}");
    let (data, ..) = file(&mut client, "paper", "web/index.js", 0)
        .await
        .expect("读得到");
    assert_eq!(data, b"export default 2;");
    let reply = client.call("c", "check", json!({})).await;
    assert!(theme_problems(&reply).is_empty(), "{reply}");
}

#[tokio::test]
async fn check_says_when_the_web_dir_is_missing() {
    let home = Home::new();
    home.write("opt/paper/package.toml", WEB_ONLY);
    let mut client = connected(&home).await;
    let reply = client.call("c", "check", json!({})).await;
    assert_eq!(
        theme_problems(&reply),
        [("theme_missing".to_string(), "包目录里没有 web".to_string())],
        "{reply}"
    );
    // 是个文件不是目录的也算不在。
    home.write("opt/paper/web", "not a dir");
    let reply = client.call("c2", "check", json!({})).await;
    assert_eq!(theme_problems(&reply).len(), 1, "{reply}");
}

/// 链接指出网页目录的不给：指出包目录的、指到包里网页目录外面的、网页目录本身指出包目录的（只在 Unix 上建得了链接不要权限）。
#[cfg(unix)]
#[tokio::test]
async fn links_out_of_the_web_dir_are_not_followed() {
    use std::os::unix::fs::symlink;
    let home = Home::new();
    home.write("opt/night/package.toml", BOTH);
    home.write("opt/night/theme.json", "{}");
    home.write("opt/night/notes.txt", "inside the package, outside web");
    home.write("opt/night/web/index.js", "1");
    home.write("opt/outside/secret.txt", "not yours");
    home.write("opt/away/package.toml", WEB_ONLY);
    let root = home.root.packages();
    symlink(
        root.join("outside/secret.txt"),
        root.join("night/web/leak.txt"),
    )
    .expect("建得了链接");
    symlink(root.join("night/notes.txt"), root.join("night/web/up.txt")).expect("建得了链接");
    symlink(
        root.join("night/web/index.js"),
        root.join("night/web/same.js"),
    )
    .expect("建得了链接");
    symlink(root.join("outside"), root.join("away/web")).expect("建得了链接");
    let mut client = connected(&home).await;
    for (id, path) in [
        ("night", "web/leak.txt"),
        ("night", "web/up.txt"),
        ("away", "web/secret.txt"),
    ] {
        let reply = file(&mut client, id, path, 0).await.expect_err("不给");
        assert_eq!(
            reply["error"]["data"]["reason"], "not_found",
            "{id} {path}：{reply}"
        );
    }
    let (data, ..) = file(&mut client, "night", "web/same.js", 0)
        .await
        .expect("网页目录里指来指去的照给");
    assert_eq!(data, b"1");
}
