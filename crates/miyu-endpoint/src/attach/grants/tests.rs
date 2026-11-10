use serde_json::json;

use super::*;
use crate::test_support::temp_root;

fn hash(text: &str) -> ContentHash {
    ContentHash::of(text.as_bytes())
}

#[tokio::test]
async fn a_package_holds_only_what_it_was_granted() {
    let grants = Grants::default();
    grants.grant("a", &hash("one")).await;
    assert!(grants.holds("a", &hash("one")));
    assert!(!grants.holds("a", &hash("two")));
    assert!(!grants.holds("b", &hash("one")));
}

#[tokio::test]
async fn an_answer_passes_on_only_what_the_answering_package_holds() {
    let grants = Grants::default();
    grants.grant("answers", &hash("own")).await;
    let answer = json!({
        "picture": hash("own").as_str(),
        "nested": [{"blob": hash("seen elsewhere").as_str()}, "not a hash", 3],
    });
    grants.pass("answers", "asks", &answer);
    assert!(grants.holds("asks", &hash("own")));
    assert!(!grants.holds("asks", &hash("seen elsewhere")));
    grants.pass("holds nothing", "asks", &json!([hash("other").as_str()]));
    assert!(!grants.holds("asks", &hash("other")));
}

#[tokio::test]
async fn a_removed_package_starts_over() {
    let grants = Grants::default();
    grants.grant("a", &hash("one")).await;
    grants.forget("a").await;
    assert!(!grants.holds("a", &hash("one")));
}

/// 人的连接交给包的参数（施工 BUS-2）：里面合哈希写法的都交过去，别的字不算。
#[test]
fn a_person_hands_over_every_hash_in_the_params() {
    let grants = Grants::default();
    grants.hand(
        "stt",
        &json!({"audio": hash("recording").as_str(), "language": "zh"}),
    );
    assert!(grants.holds("stt", &hash("recording")));
    grants.hand("stt", &json!({"text": "not a hash"}));
    assert!(!grants.holds("other", &hash("recording")));
}

/// 落盘的那一份里记着的行（施工 BG-1）；没有这份文件的是空的。
fn saved_lines(root: &DataRoot, package: &str) -> Vec<String> {
    std::fs::read_to_string(root.blob_grants(package))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// 自己存的落盘（施工 BG-1）：同一个数据根上新起的一份读得回；同一个哈希只写一行。
#[tokio::test]
async fn what_a_package_stored_itself_survives_a_restart() {
    let (root, _dir) = temp_root("grants-saved");
    let grants = Grants::load(&root);
    grants.grant("memes", &hash("one")).await;
    grants.grant("memes", &hash("one")).await;
    grants.grant("memes", &hash("two")).await;
    assert_eq!(
        saved_lines(&root, "memes"),
        [hash("one").as_str(), hash("two").as_str()],
        "一行一个，存过的不再写"
    );
    let again = Grants::load(&root);
    assert!(again.holds("memes", &hash("one")));
    assert!(again.holds("memes", &hash("two")));
    assert!(!again.holds("other", &hash("one")), "别的包照旧拿不到");
    // 读回来的也算存过：再存不多写一行。
    again.grant("memes", &hash("one")).await;
    assert_eq!(saved_lines(&root, "memes").len(), 2);
}

/// 经 `package.call` 转来的、人交来的只在内存里（施工 BG-1）：那是一次调用里临时给的，重起就没了。后来自己又存了的照样落盘。
#[tokio::test]
async fn what_was_passed_or_handed_is_not_saved() {
    let (root, _dir) = temp_root("grants-passed");
    let grants = Grants::load(&root);
    grants.grant("memes", &hash("own")).await;
    grants.pass("memes", "bridge", &json!({"picture": hash("own").as_str()}));
    grants.hand("stt", &json!({"audio": hash("recording").as_str()}));
    assert!(grants.holds("bridge", &hash("own")) && grants.holds("stt", &hash("recording")));
    assert!(saved_lines(&root, "bridge").is_empty() && saved_lines(&root, "stt").is_empty());
    let again = Grants::load(&root);
    assert!(again.holds("memes", &hash("own")));
    assert!(!again.holds("bridge", &hash("own")), "转来的重起就没了");
    assert!(!again.holds("stt", &hash("recording")), "交来的重起就没了");

    grants.grant("bridge", &hash("own")).await;
    assert_eq!(saved_lines(&root, "bridge"), [hash("own").as_str()]);
    assert!(Grants::load(&root).holds("bridge", &hash("own")));
}

/// 卸了的包（施工 BG-1）：落盘的那一份删掉，重起以后也拿不到；别的包的不动。
#[tokio::test]
async fn a_removed_package_leaves_no_file() {
    let (root, _dir) = temp_root("grants-forgotten");
    let grants = Grants::load(&root);
    grants.grant("memes", &hash("one")).await;
    grants.grant("voice", &hash("two")).await;
    grants.forget("memes").await;
    assert!(!grants.holds("memes", &hash("one")));
    assert!(!root.blob_grants("memes").exists());
    let again = Grants::load(&root);
    assert!(!again.holds("memes", &hash("one")));
    assert!(again.holds("voice", &hash("two")));
    // 装回来再存：从空的开始，重新写。
    grants.grant("memes", &hash("one")).await;
    assert_eq!(saved_lines(&root, "memes"), [hash("one").as_str()]);
}

/// 坏行跳过（施工 BG-1）：好的行照收。
#[tokio::test]
async fn bad_lines_in_the_file_are_skipped() {
    let (root, _dir) = temp_root("grants-bad");
    Grants::load(&root).grant("memes", &hash("one")).await;
    let path = root.blob_grants("memes");
    let kept = std::fs::read_to_string(&path).expect("写下了");
    std::fs::write(&path, format!("not a hash\n{kept}sha256:12")).expect("写得进");
    let again = Grants::load(&root);
    assert!(again.holds("memes", &hash("one")));
    again.grant("memes", &hash("two")).await;
    assert!(
        Grants::load(&root).holds("memes", &hash("two")),
        "半行后面接着记的读得回"
    );
}

/// 写不进的（数据根不在了）存照算成：这一次起来照旧拿得到。
#[tokio::test]
async fn a_grant_that_cannot_be_saved_still_holds() {
    let (root, dir) = temp_root("grants-gone");
    let grants = Grants::load(&root);
    drop(dir);
    grants.grant("memes", &hash("one")).await;
    assert!(grants.holds("memes", &hash("one")));
    assert!(!root.path().exists(), "不把数据根建回来");
}
