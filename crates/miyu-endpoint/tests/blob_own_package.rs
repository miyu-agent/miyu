//! 扩展经 `blob.put` 的 `path` 存它自己包里带的文件（施工 W-6 四补，终端报的）：包目录在数据根里，原来一律拒
//! `attachment_in_data_root`，作者只能改成读出来再传 base64。现在真实位置在调用的这个扩展自己的包目录里的照收；别的包的、
//! 自己包里一个链接指到外面的照旧拒。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

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

/// 存成了的 blob；拒了的是拒绝的原因。
fn stored(reply: &Value) -> Result<String, String> {
    match reply["result"]["blob"].as_str() {
        Some(blob) => Ok(blob.to_string()),
        None => Err(reply["error"]["data"]["reason"]
            .as_str()
            .unwrap_or_default()
            .to_string()),
    }
}

fn put(path: &Path) -> String {
    format!(
        r#"ask:blob.put:{{"path":{}}}"#,
        serde_json::to_string(&path.to_string_lossy()).expect("写得成 JSON")
    )
}

#[tokio::test]
async fn an_extension_stores_files_from_its_own_package_folder_only() {
    let home = Home::new();
    let packages = home.root.path().join("home/alice/packages");
    let own = packages.join("memes/files/cat.png");
    let other = packages.join("other/files/secret.txt");
    std::fs::create_dir_all(own.parent().expect("有上一层")).expect("建得了");
    std::fs::create_dir_all(other.parent().expect("有上一层")).expect("建得了");
    std::fs::write(&own, b"\x89PNG\r\n\x1a\n cat").expect("写得进");
    std::fs::write(&other, b"other package").expect("写得进");
    let mut asks = vec![put(&own), put(&other)];
    // 自己包里一个链接，指到别的包：照真实位置判，照旧拒。
    #[cfg(unix)]
    {
        let link = packages.join("memes/files/link.txt");
        std::os::unix::fs::symlink(&other, &link).expect("建得了链接");
        asks.push(put(&link));
    }
    let program = Program::new();
    let (log, step) = record(&home, "memes");
    let mut all = vec![step.as_str(), "hello"];
    all.extend(asks.iter().map(String::as_str));
    all.push("wait");
    install(&home, "memes", &program.name(), "always", &steps(&all));
    let script = Script::new([]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();

    let got = replies(&log, 1 + asks.len()).await;
    let answers: Vec<Result<String, String>> = got[1..].iter().map(stored).collect();

    assert_eq!(
        answers[0],
        Ok(ContentHash::of(b"\x89PNG\r\n\x1a\n cat")
            .as_str()
            .to_string()),
        "自己包里的照收"
    );
    assert_eq!(
        answers[1],
        Err("attachment_in_data_root".to_string()),
        "别的包的照旧拒"
    );
    if answers.len() > 2 {
        assert_eq!(
            answers[2],
            Err("attachment_in_data_root".to_string()),
            "链接指到外面的照旧拒"
        );
    }
    core.stop_extensions().await;
}
