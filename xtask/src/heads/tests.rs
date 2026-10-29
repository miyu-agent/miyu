//! `heads.toml` 的写法。

use super::*;

const GOOD: &str = r#"
[miyu-tui]
repo = "https://github.com/miyu-agent/miyu-tui"
rev = "960362426789aaea2d34242711cfc6d301e45ca9"

[miyu-webui]
repo = "https://github.com/miyu-agent/miyu-web"
rev = "v0.0.1"
"#;

#[test]
fn a_commit_or_a_tag_under_the_org_is_fine() {
    assert_eq!(problems(GOOD), Vec::<String>::new());
}

#[test]
fn the_one_in_the_repository_is_fine() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    assert_eq!(check(&root), Vec::<String>::new());
}

#[test]
fn each_mistake_is_named() {
    let short = GOOD.replace("960362426789aaea2d34242711cfc6d301e45ca9", "9603624");
    assert_eq!(
        problems(&short).len(),
        1,
        "短的提交号不算钉死：{:?}",
        problems(&short)
    );
    let elsewhere = GOOD.replace("miyu-agent/miyu-web", "someone/miyu-web");
    assert_eq!(problems(&elsewhere).len(), 1);
    let branch = GOOD.replace("v0.0.1", "main");
    assert_eq!(problems(&branch).len(), 1);
    let missing = GOOD.replace("[miyu-webui]", "[gui]");
    assert_eq!(
        problems(&missing).len(),
        2,
        "多了一张、少了一张：{:?}",
        problems(&missing)
    );
    let extra = GOOD.replace("rev = \"v0.0.1\"", "rev = \"v0.0.1\"\nbranch = \"main\"");
    assert_eq!(problems(&extra).len(), 1);
    assert_eq!(problems("not toml [").len(), 1);
}
