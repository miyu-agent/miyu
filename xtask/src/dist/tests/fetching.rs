//! 取头的测试（从 `tests.rs` 挪来，施工 LW-6：那个文件到了行数上限）：照钉的提交检出、清掉多余的文件、留着 target。

use super::*;

/// 在 `dir` 里跑一条 git，要成功，交回输出（去掉末尾换行）。
fn git_in(dir: &Path, args: &[&str]) -> String {
    let done = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("git 起得来");
    assert!(
        done.status.success(),
        "git {args:?}：{}",
        String::from_utf8_lossy(&done.stderr)
    );
    String::from_utf8(done.stdout)
        .unwrap()
        .trim_end()
        .to_string()
}

#[test]
fn fetching_checks_out_the_pin_cleans_strays_and_keeps_target() {
    let scratch = Scratch::new("fetch");
    let upstream = scratch.0.join("upstream");
    fs::create_dir_all(&upstream).unwrap();
    git_in(&upstream, &["init", "--quiet", "-b", "main"]);
    scratch.put("upstream/tui/package.toml", "one");
    git_in(&upstream, &["add", "-A"]);
    git_in(&upstream, &["commit", "--quiet", "-m", "one"]);
    let first = git_in(&upstream, &["rev-parse", "HEAD"]);
    git_in(&upstream, &["tag", "v0.1.0"]);
    let dir = scratch.0.join("heads/tui");
    let pinned = |rev: &str| Pin {
        id: "tui".to_string(),
        repo: upstream.to_string_lossy().into_owned(),
        rev: rev.to_string(),
    };
    fetch(&dir, &pinned(&first)).unwrap();
    assert_eq!(git_in(&dir, &["rev-parse", "HEAD"]), first);
    assert_eq!(
        fs::read_to_string(dir.join("tui/package.toml")).unwrap(),
        "one"
    );
    assert_eq!(
        tracked(&dir, "tui").unwrap(),
        vec![PathBuf::from("tui/package.toml")]
    );

    // 上游在克隆以后又提交了一份：要先拉才检得出来。
    scratch.put("upstream/tui/package.toml", "two");
    scratch.put("upstream/tui/extra.json", "{}");
    git_in(&upstream, &["add", "-A"]);
    git_in(&upstream, &["commit", "--quiet", "-m", "two"]);
    let second = git_in(&upstream, &["rev-parse", "HEAD"]);

    // 再取一次换成第二份：拉、检出、清掉多余的，target/ 留着。
    scratch.put("heads/tui/stray.txt", "left over");
    scratch.put("heads/tui/target/release/keep", "built");
    fetch(&dir, &pinned(&second)).unwrap();
    assert_eq!(git_in(&dir, &["rev-parse", "HEAD"]), second);
    assert!(!dir.join("stray.txt").exists(), "多余的清掉");
    assert!(dir.join("target/release/keep").exists(), "target/ 留着");
    assert_eq!(
        tracked(&dir, "tui").unwrap(),
        vec![
            PathBuf::from("tui/extra.json"),
            PathBuf::from("tui/package.toml")
        ]
    );

    // 标签也认；钉的不在的报错。
    fetch(&dir, &pinned("v0.1.0")).unwrap();
    assert_eq!(git_in(&dir, &["rev-parse", "HEAD"]), first);
    assert!(fetch(&dir, &pinned("v9.9.9")).is_err());
}
