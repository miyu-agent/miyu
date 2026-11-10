//! 升级后第一次起来，把包以前存的 blob 补登记给它（施工 BG-1补，`protocol.md` 的 `blob.get` 第 1 条）：BG-1 之前存的，核心
//! 从来没记过是谁存的；表情包这样的包把图只存在核心里、自己的状态目录里只记哈希。数据根里还没有 `var/lib/blob-grants/`
//! 的那一次起来，核心照各个包状态目录里写着的哈希补登记，只做这一次：以后再往状态目录里写别处看来的哈希开不了门。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use miyu_endpoint::Core;
use miyu_kernel::id::ContentHash;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 等到记下的回应有 `n` 行（`cwd:` 那一行不算），交回它们。
async fn replies(path: &Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<Value> = read(path)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 读成的内容；拒了的是拒绝的原因。
fn got(reply: &Value) -> Result<Vec<u8>, String> {
    match reply["result"]["data"].as_str() {
        Some(data) => Ok(STANDARD.decode(data).expect("是 base64")),
        None => Err(reply["error"]["data"]["reason"]
            .as_str()
            .unwrap_or_default()
            .to_string()),
    }
}

fn put(content: &[u8]) -> String {
    format!(
        r#"ask:blob.put:{{"data":"{}","name":"m.png"}}"#,
        STANDARD.encode(content)
    )
}

fn get(content: &[u8]) -> String {
    format!(
        r#"ask:blob.get:{{"blob":"{}"}}"#,
        ContentHash::of(content).as_str()
    )
}

/// 一份核心，照 `miyu-core` 起来的先后：先补登记，再拉起扩展。
async fn started(home: &Home) -> Arc<Core> {
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.backfill_blob_grants().await;
    core.start_extensions();
    core
}

/// 包 `id` 在自己的状态目录里记下 `content` 的哈希（表情包的索引就是这样一份 JSON）。
fn note(home: &Home, id: &str, file: &str, content: &[u8]) {
    let dir = home.root.package_state(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(
        dir.join(file),
        format!(
            r#"{{"memes":[{{"blob":"{}"}}]}}"#,
            ContentHash::of(content).as_str()
        ),
    )
    .expect("写得进");
}

#[tokio::test]
async fn what_a_package_noted_before_grants_were_saved_is_granted_once() {
    let home = Home::new();
    let (noted, later) = (&b"a meme stored before BG-1"[..], &b"another blob"[..]);
    let (memes, other) = (Program::new(), Program::new());
    let (memes_log, step) = record(&home, "memes");
    install(
        &home,
        "memes",
        &memes.name(),
        "always",
        &steps(&[&step, "hello", &put(noted), &put(later), "wait"]),
    );
    let first = started(&home).await;
    let stored = replies(&memes_log, 3).await;
    assert!(stored[2]["result"]["blob"].is_string(), "{stored:?}");
    first.stop_extensions().await;
    drop(first);

    // BG-1 之前的数据根：包只在自己的状态目录里记着哈希，核心没有记过谁存的。
    note(&home, "memes", "index.json", noted);
    std::fs::remove_dir_all(home.root.blob_grants_dir()).expect("删得掉");
    install(
        &home,
        "memes",
        &memes.name(),
        "always",
        &steps(&[
            &step,
            "ids:again",
            "hello",
            &get(noted),
            &get(later),
            "wait",
        ]),
    );
    let (other_log, other_step) = record(&home, "other");
    install(
        &home,
        "other",
        &other.name(),
        "always",
        &steps(&[&other_step, "hello", &get(noted), "wait"]),
    );
    let second = started(&home).await;
    let read_memes = replies(&memes_log, 6).await;
    assert_eq!(
        got(&read_memes[4]),
        Ok(noted.to_vec()),
        "状态目录里记着的补登记给它：{read_memes:?}"
    );
    assert_eq!(
        got(&read_memes[5]),
        Err("unknown_blob".to_string()),
        "状态目录里没记的不补：{read_memes:?}"
    );
    let read_other = replies(&other_log, 2).await;
    assert_eq!(
        got(&read_other[1]),
        Err("unknown_blob".to_string()),
        "别的包的状态目录里没有，读不到：{read_other:?}"
    );
    assert!(home.root.blob_grants_dir().is_dir(), "目录建出来了");
    second.stop_extensions().await;
    drop(second);

    // 只做一次：目录在了以后再往状态目录里写别的存在的哈希，开不了门。
    note(&home, "memes", "more.json", later);
    install(
        &home,
        "memes",
        &memes.name(),
        "always",
        &steps(&[
            &step,
            "ids:third",
            "hello",
            &get(later),
            &get(noted),
            "wait",
        ]),
    );
    let third = started(&home).await;
    let read_again = replies(&memes_log, 9).await;
    assert_eq!(
        got(&read_again[7]),
        Err("unknown_blob".to_string()),
        "后来写进去的不补：{read_again:?}"
    );
    assert_eq!(
        got(&read_again[8]),
        Ok(noted.to_vec()),
        "补过的落了盘，重起以后还在：{read_again:?}"
    );
    third.stop_extensions().await;
}
