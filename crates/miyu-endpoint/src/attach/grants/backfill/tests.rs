//! 补登记（施工 BG-1补）：状态目录里记着、存处有的登记给那个包、落盘；存处没有的不登记；目录建出来、只做一次；一个包都
//! 没有的也建目录；一个包的状态目录读不了的不拦别的包。

use std::fs;

use super::*;
use crate::test_support::temp_root;

/// 存进 `root` 的存处，交回哈希。
fn stored(root: &DataRoot, content: &str) -> ContentHash {
    Blobs::new(root.blobs())
        .put(content.as_bytes())
        .expect("存得进")
}

/// 包 `package` 在自己的状态目录里记下 `hashes`。
fn note(root: &DataRoot, package: &str, file: &str, hashes: &[&ContentHash]) {
    let dir = root.package_state(package);
    fs::create_dir_all(&dir).expect("建得了");
    let lines: Vec<&str> = hashes.iter().map(|hash| hash.as_str()).collect();
    fs::write(dir.join(file), lines.join("\n")).expect("写得进");
}

fn packages(ids: &[&str]) -> Vec<String> {
    ids.iter().map(ToString::to_string).collect()
}

#[tokio::test]
async fn what_a_package_noted_and_the_store_has_is_granted_and_saved() {
    let (root, _dir) = temp_root("backfill");
    let (have, other) = (stored(&root, "have"), stored(&root, "other"));
    let gone = ContentHash::of(b"never stored");
    note(&root, "memes", "index.json", &[&have, &gone]);
    let grants = Grants::load(&root);
    grants.backfill(packages(&["memes", "voice"])).await;
    assert!(grants.holds("memes", &have), "记着、存处有的");
    assert!(!grants.holds("memes", &gone), "存处没有的不登记");
    assert!(!grants.holds("memes", &other), "没记着的不登记");
    assert!(!grants.holds("voice", &have), "别的包没记着");
    assert_eq!(
        fs::read_to_string(root.blob_grants("memes")).expect("落了盘"),
        format!("{}\n", have.as_str())
    );
    assert!(!root.blob_grants("voice").exists(), "没补到的包没有文件");
    // 重起以后还在；它自己再存同一个不多写一行。
    let again = Grants::load(&root);
    assert!(again.holds("memes", &have));
    grants.grant("memes", &have).await;
    assert_eq!(
        fs::read_to_string(root.blob_grants("memes")).expect("读得了"),
        format!("{}\n", have.as_str()),
        "同一个不重复"
    );
}

#[tokio::test]
async fn it_runs_only_while_the_directory_is_missing() {
    let (root, _dir) = temp_root("backfill-once");
    let grants = Grants::load(&root);
    grants.backfill(Vec::new()).await;
    assert!(
        root.blob_grants_dir().is_dir(),
        "一个包都没有也把目录建出来"
    );
    // 目录在了：后来写进状态目录的不认，这一次、重起以后都不认。
    let late = stored(&root, "late");
    note(&root, "memes", "index.json", &[&late]);
    grants.backfill(packages(&["memes"])).await;
    assert!(!grants.holds("memes", &late));
    let again = Grants::load(&root);
    again.backfill(packages(&["memes"])).await;
    assert!(!again.holds("memes", &late));
    assert!(!root.blob_grants("memes").exists());
}

#[tokio::test]
async fn a_package_whose_state_cannot_be_scanned_does_not_stop_the_others() {
    let (root, _dir) = temp_root("backfill-bad");
    let have = stored(&root, "have");
    // 状态目录的位置上是一份文件：扫不了。
    fs::create_dir_all(root.package_states()).expect("建得了");
    fs::write(root.package_state("broken"), have.as_str()).expect("写得进");
    note(&root, "memes", "index.json", &[&have]);
    let grants = Grants::load(&root);
    grants.backfill(packages(&["broken", "memes"])).await;
    assert!(!grants.holds("broken", &have));
    assert!(grants.holds("memes", &have), "别的包照补");
    assert!(root.blob_grants_dir().is_dir());
}

#[tokio::test]
async fn grants_kept_only_in_memory_are_not_backfilled() {
    let grants = Grants::default();
    grants.backfill(packages(&["memes"])).await;
    assert!(!grants.holds("memes", &ContentHash::of(b"x")));
}

#[tokio::test]
async fn a_half_written_staging_directory_is_thrown_away_and_the_backfill_done_in_full() {
    let (root, _dir) = temp_root("backfill-half");
    let (have, more) = (stored(&root, "have"), stored(&root, "more"));
    note(&root, "memes", "index.json", &[&have]);
    note(&root, "voice", "state", &[&more]);
    // 上一次写到一半被杀：暂存目录在，里面只有一个包的半份、还有一个现在没装的包的；正式的目录没有。
    let staged = root.blob_grants_staging();
    fs::create_dir_all(&staged).expect("建得了");
    fs::write(staged.join("memes"), "sha256:0123").expect("写得进");
    fs::write(staged.join("gone"), format!("{}\n", more.as_str())).expect("写得进");
    let grants = Grants::load(&root);
    grants.backfill(packages(&["memes", "voice"])).await;
    assert!(grants.holds("memes", &have) && grants.holds("voice", &more));
    assert!(!staged.exists(), "暂存目录没了");
    assert_eq!(
        fs::read_to_string(root.blob_grants("memes")).expect("落了盘"),
        format!("{}\n", have.as_str()),
        "半份不留"
    );
    assert_eq!(
        fs::read_to_string(root.blob_grants("voice")).expect("落了盘"),
        format!("{}\n", more.as_str())
    );
    assert!(!root.blob_grants("gone").exists(), "上次剩下的不带进来");
    let again = Grants::load(&root);
    assert!(again.holds("memes", &have) && again.holds("voice", &more));
    assert!(!again.holds("gone", &more));
}
