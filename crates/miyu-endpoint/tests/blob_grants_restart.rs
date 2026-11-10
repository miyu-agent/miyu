//! 包自己存的 blob，核心重起以后还读得到（施工 BG-1，`protocol.md` 的 `blob.get` 第 1 条）：表情包这样的包把图只存在核心的
//! blob 里、自己只记哈希，重起以后手里没有内容，没法「再存一次」。它自己存过哪些记在数据根的 `var/lib/blob-grants/<包>`，
//! 核心起来时读回；别的包照旧读不到。同一个数据根上换一份核心，就像重起过（同 `restart.rs`）。

use std::path::Path;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use miyu_kernel::id::ContentHash;

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

#[tokio::test]
async fn a_package_reads_back_what_it_stored_before_the_core_restarted() {
    let home = Home::new();
    let picture = &b"a meme only the core keeps"[..];
    let hash = ContentHash::of(picture).as_str().to_string();
    let get = format!(r#"ask:blob.get:{{"blob":"{hash}"}}"#);
    let (memes, other) = (Program::new(), Program::new());
    let (memes_log, step) = record(&home, "memes");
    let store = format!(
        r#"ask:blob.put:{{"data":"{}","name":"m.png"}}"#,
        STANDARD.encode(picture)
    );
    install(
        &home,
        "memes",
        &memes.name(),
        "always",
        &steps(&[&step, "hello", &store, "wait"]),
    );
    let first = core(&home, quick());
    let stored = replies(&memes_log, 2).await;
    assert_eq!(stored[1]["result"]["blob"], hash.as_str(), "{stored:?}");
    first.stop_extensions().await;
    drop(first);

    // 重起以后：存的那个包只读、不再存；另一个包拿同一个哈希来读。请求编号换一个打头的，不和上一次拉起的撞。
    install(
        &home,
        "memes",
        &memes.name(),
        "always",
        &steps(&[&step, "ids:again", "hello", &get, "wait"]),
    );
    let (other_log, step) = record(&home, "other");
    install(
        &home,
        "other",
        &other.name(),
        "always",
        &steps(&[&step, "hello", &get, "wait"]),
    );
    let second = core(&home, quick());
    let read_memes = replies(&memes_log, 4).await;
    assert_eq!(
        got(&read_memes[3]),
        Ok(picture.to_vec()),
        "自己以前存的，核心重起以后还读得到：{read_memes:?}"
    );
    let read_other = replies(&other_log, 2).await;
    assert_eq!(
        got(&read_other[1]),
        Err("unknown_blob".to_string()),
        "别的包照旧读不到：{read_other:?}"
    );
    second.stop_extensions().await;
}
