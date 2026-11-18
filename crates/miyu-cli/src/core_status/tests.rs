use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use miyu_ipc::{Dirs, Lock};
use miyu_store::env::{Env, Platform};

use super::*;

/// 一个用完就删的临时数据根，建好了骨架。
fn temp_root(name: &str) -> DataRoot {
    let dir = std::env::temp_dir().join(format!(
        "miyu-cli-core-status-{name}-{}",
        std::process::id()
    ));
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(dir.into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("建得了骨架");
    root
}

/// 问一次，交回退出码、标准输出、标准错误。
async fn asked(root: &DataRoot, format: Format, language: Language) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = core_status_on(root, format, language, &mut out, &mut err).await;
    let text = |bytes: Vec<u8>| String::from_utf8(bytes).expect("是字");
    (code, text(out), text(err))
}

/// 起一个假核心：答 `hello`，说自己是 `version` 这一版；连接断了就走。
fn fake_core(root: &DataRoot, version: &'static str) -> tokio::task::JoinHandle<Vec<String>> {
    let opened = miyu_ipc::open_locked(
        root,
        &Dirs::current(),
        Lock::acquire(root).expect("拿得到锁"),
    )
    .expect("开得了套接字");
    tokio::spawn(async move {
        let mut listener = opened.listener;
        let connection = listener.accept().await.expect("连得上");
        let (read, mut write) = tokio::io::split(connection);
        let mut lines = BufReader::new(read).lines();
        let mut asked = Vec::new();
        while let Some(line) = lines.next_line().await.expect("读得了") {
            let request: Value = serde_json::from_str(&line).expect("是 JSON");
            asked.push(request["method"].as_str().unwrap_or_default().to_string());
            let result = json!({"core": {"version": version}, "language": "zh", "protocol": 1});
            let reply = json!({"jsonrpc": "2.0", "id": request["id"], "result": result});
            write
                .write_all(format!("{reply}\n").as_bytes())
                .await
                .expect("写得出");
        }
        asked
    })
}

/// 没在跑的：说没在跑、数据根在哪、怎么起，退出码 3；不拉起（锁还拿得到）。
#[tokio::test]
async fn nothing_running_says_so_with_code_three() {
    let root = temp_root("idle");
    let place = root.path().display().to_string();
    let (code, out, err) = asked(&root, Format::Text, Language::Chinese).await;
    assert_eq!(code, NOT_RUNNING, "{err}");
    assert_eq!(
        out,
        format!("核心没有在运行\n数据根：{place}\n提示：运行 'miyu core start' 启动\n")
    );
    assert!(err.is_empty(), "{err}");
    let (code, out, _) = asked(&root, Format::Text, Language::English).await;
    assert_eq!(code, NOT_RUNNING);
    assert_eq!(
        out,
        format!(
            "The core is not running\ndata root: {place}\nhint: run 'miyu core start' to start it\n"
        )
    );
    let (code, out, _) = asked(&root, Format::Json, Language::Chinese).await;
    assert_eq!(code, NOT_RUNNING, "机读的退出码一样");
    assert_eq!(
        serde_json::from_str::<Value>(&out).expect("一行 JSON"),
        json!({"running": false, "version": null, "data_root": place})
    );
    Lock::acquire(&root).expect("只看，没拉起");
    std::fs::remove_dir_all(root.path()).expect("删得掉临时目录");
}

/// 在跑的：握手，版本照核心报的（不是这个程序自己的），退出码 0。
#[tokio::test]
async fn a_running_core_reports_its_own_version() {
    let root = temp_root("running");
    let place = root.path().display().to_string();
    let core = fake_core(&root, "2026.1.2");
    let (code, out, err) = asked(&root, Format::Text, Language::English).await;
    assert_eq!(code, exit::OK, "{err}");
    assert_eq!(
        out,
        format!("核心正在运行\n版本：2026.1.2\n数据根：{place}\n"),
        "握手以后照核心说的语言"
    );
    assert!(err.is_empty(), "{err}");
    assert_eq!(core.await.expect("没崩"), ["hello"]);
    std::fs::remove_dir_all(root.path()).expect("删得掉临时目录");
}

#[tokio::test]
async fn a_running_core_in_json() {
    let root = temp_root("json");
    let place = root.path().display().to_string();
    let core = fake_core(&root, "2026.1.2");
    let (code, out, err) = asked(&root, Format::Json, Language::Chinese).await;
    assert_eq!(code, exit::OK, "{err}");
    assert_eq!(out.lines().count(), 1, "{out}");
    assert_eq!(
        serde_json::from_str::<Value>(&out).expect("一行 JSON"),
        json!({"running": true, "version": "2026.1.2", "data_root": place})
    );
    core.await.expect("没崩");
    std::fs::remove_dir_all(root.path()).expect("删得掉临时目录");
}
