//! 画 mermaid 时字体只记在哪、不存副本（施工 V-35，`docs/blueprint/mermaid.md`「怎么走」第 2 条）：真的 `miyu core`，画一张
//! 图以后数据根的 `cache/mermaid` 里只有几十字节的位置记录，没有拷下来的字体；画图的库原来写的 `$XDG_CACHE_HOME/mmdr` 一个
//! 字都不写。重起核心照记下的位置再画，出来的 SVG 和头一回一字不差。

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncReader};

use crate::support::{Home, within};

const SOURCE: &str = "flowchart LR\n  A[开始 Start] --> B{判断 Check}\n  B -->|是| C[Café]";

/// 拉起核心，等它写来 `ready`。
fn start(home: &Home) -> Child {
    let mut core = home
        .core_staying()
        .stdout(Stdio::piped())
        .spawn()
        .expect("起得来");
    let mut line = String::new();
    BufReader::new(core.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到那一行");
    assert_eq!(line, "ready\n", "核心起来了：{}", home.core_log());
    core
}

#[expect(
    clippy::let_underscore_must_use,
    reason = "收拾核心是测试收尾，停不掉也不影响这一条测试的结论"
)]
fn stop(core: &mut Child) {
    #[cfg(unix)]
    let stopped = Command::new("kill").arg(core.id().to_string()).status();
    #[cfg(windows)]
    let stopped = Command::new("taskkill")
        .args(["/PID", &core.id().to_string(), "/F"])
        .status();
    let _ = stopped;
    let _ = core.wait();
}

/// 连上、握手、画一张，交回 SVG。
async fn draw(home: &Home) -> String {
    let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
        .await
        .expect("连得上");
    let (read, mut write) = tokio::io::split(connection);
    let mut read = AsyncReader::new(read);
    let mut call = async |id: &str, method: &str, params: Value| -> Value {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("写得进");
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                let mut line = String::new();
                assert!(
                    read.read_line(&mut line).await.expect("读得了") > 0,
                    "核心断开了"
                );
                let reply: Value = serde_json::from_str(&line).expect("是 JSON");
                if reply["id"] == id {
                    return reply;
                }
            }
        })
        .await
        .expect("一分钟内等到回应")
    };
    let hello = call(
        "h1",
        "hello",
        json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"}, "token": token}),
    )
    .await;
    assert!(hello.get("error").is_none(), "{hello}");
    let reply = call("m1", "mermaid.render", json!({"source": SOURCE})).await;
    reply["result"]["svg"]
        .as_str()
        .unwrap_or_else(|| panic!("画得出：{reply}"))
        .to_string()
}

/// 照画图的库的查法（fontdb，`style.json` 那几种名字，最后一个 `sans-serif` 照通用的无衬线）看这台机器上有没有对得上的字体。
fn has_style_font() -> bool {
    let style =
        std::fs::read_to_string(crate::support::resources().join("software/mermaid/style.json"))
            .expect("读得到 style.json");
    let style: Value = serde_json::from_str(&style).expect("是 JSON");
    let names: Vec<String> = style["fonts"]
        .as_array()
        .expect("fonts 是数组")
        .iter()
        .filter_map(|font| font.as_str().map(str::to_string))
        .collect();
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let families: Vec<fontdb::Family<'_>> = names
        .iter()
        .map(|name| match name.as_str() {
            "sans-serif" => fontdb::Family::SansSerif,
            name => fontdb::Family::Name(name),
        })
        .collect();
    db.query(&fontdb::Query {
        families: &families,
        ..fontdb::Query::default()
    })
    .is_some()
}

/// 目录里每个文件的名字和大小。
fn files(dir: &Path) -> Vec<(String, u64)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, u64)> = entries
        .flatten()
        .map(|entry| {
            let size = entry.metadata().map_or(0, |meta| meta.len());
            (entry.file_name().to_string_lossy().into_owned(), size)
        })
        .collect();
    found.sort();
    found
}

#[tokio::test]
async fn fonts_are_remembered_by_where_they_are_not_copied() {
    let home = Home::new();
    let mut core = start(&home);
    let first = draw(&home).await;
    stop(&mut core);
    let remembered = files(&home.root.mermaid_cache());
    // 这台机器上一种都对不上的（CI 的 ubuntu 没装 `style.json` 那几种，`sans-serif` 默认对的 Arial 也没有），画图的库照字数估
    // 宽，没有位置可记；对得上的必须记下。
    assert_eq!(
        !remembered.is_empty(),
        has_style_font(),
        "对得上字体就记下它在哪：{remembered:?}"
    );
    for (name, size) in &remembered {
        assert!(
            Path::new(name).extension().is_some_and(|e| e == "location"),
            "只有位置记录：{remembered:?}"
        );
        assert!(*size < 4096, "不是拷下来的字体：{remembered:?}");
    }
    let shared = crate::support::offline(home.root.path())[1].1.join("mmdr");
    assert!(
        !shared.exists(),
        "画图的库默认的缓存目录一个字都不写：{shared:?}"
    );
    let mut core = start(&home);
    let again = draw(&home).await;
    stop(&mut core);
    assert_eq!(first, again, "照记下的位置画的和头一回一字不差");
    assert_eq!(files(&home.root.mermaid_cache()), remembered);
}
