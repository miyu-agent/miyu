//! 版本号的写法（施工 VR-1）：年.月.序号，月、序号不补零；改工作区版本只动 `[workspace.package]` 那一行；`release.yml` 照同一条正则查。

use super::*;

#[test]
fn only_year_month_and_number_without_padding_pass() {
    for good in [
        "2026.11.0",
        "2026.1.0",
        "2026.12.3",
        "2027.10.12",
        "9999.12.100",
    ] {
        assert_eq!(check(good), Ok(()), "{good}");
    }
    for bad in [
        "0.0.0",
        "1.0.0",
        "26.11.0",
        "02026.11.0",
        "2026.011.0",
        "2026.01.0",
        "2026.0.0",
        "2026.13.0",
        "2026.11.00",
        "2026.11.01",
        "2026.11",
        "2026.11.0.1",
        "2026.11.0-rc.1",
        "v2026.11.0",
        "2026.11.x",
        "2026..0",
        "",
    ] {
        assert!(check(bad).is_err(), "{bad} 不该过");
    }
    // 说清要什么写法。
    assert!(check("2026.01.0").unwrap_err().contains("不补零"));
}

#[test]
fn only_the_workspace_package_version_line_is_rewritten() {
    let manifest = "[workspace]\nresolver = \"3\"\n\n[workspace.package]\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace.dependencies]\nversion = \"9\"\n";
    let rewritten = rewrite(manifest, "2026.11.0").expect("改得了");
    assert_eq!(
        rewritten,
        manifest.replace("version = \"0.0.0\"", "version = \"2026.11.0\"")
    );
    assert_eq!(workspace_version(&rewritten).as_deref(), Some("2026.11.0"));
    // Windows 的换行原样留着。
    let crlf = manifest.replace('\n', "\r\n");
    assert_eq!(
        rewrite(&crlf, "2026.11.0").expect("改得了"),
        crlf.replace("version = \"0.0.0\"", "version = \"2026.11.0\"")
    );
    // 没有那一行的、有两行的：报错。
    assert!(rewrite("[workspace.package]\nedition = \"2024\"\n", "2026.11.0").is_err());
    assert!(
        rewrite(
            "[workspace.package]\nversion = \"1\"\nversion = \"2\"\n",
            "2026.11.0"
        )
        .is_err()
    );
}

#[test]
fn the_lock_version_is_the_line_after_the_name() {
    let lock = "[[package]]\nname = \"miyu-call\"\nversion = \"0.0.0\"\n\n[[package]]\nname = \"miyu\"\nversion = \"2026.11.0\"\ndependencies = []\n";
    assert_eq!(locked_version(lock, "miyu").as_deref(), Some("2026.11.0"));
    assert_eq!(locked_version(lock, "miyu-call").as_deref(), Some("0.0.0"));
    assert_eq!(locked_version(lock, "nope"), None);
}

/// 这个仓库自己的 `Cargo.toml`、`Cargo.lock` 读得出版本，两边一样。
#[test]
fn this_workspace_s_version_reads_the_same_from_both_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("读得到");
    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("读得到");
    let version = workspace_version(&manifest).expect("读得出");
    // 定了号以后（施工 VR-2）工作区的版本照规矩写：release.yml 打包前也照这个查。
    assert_eq!(check(&version), Ok(()), "{version}");
    assert_eq!(locked_version(&lock, "miyu"), Some(version));
}

/// `release.yml` 打包前照同一条正则查版本。
#[test]
fn the_release_workflow_checks_the_same_pattern() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow = fs::read_to_string(root.join(".github/workflows/release.yml")).expect("读得到");
    assert!(
        workflow.contains(&format!("=~ {PATTERN} ]]")),
        "release.yml 里要有 [[ \"${{version}}\" =~ {PATTERN} ]]"
    );
}

/// 写法不对的、没写版本的：一个字都不改就停下，也不跑 cargo（给的是一个跑了就会失败的程序）。
#[test]
fn a_wrong_version_stops_before_touching_anything() {
    let root = std::env::temp_dir().join(format!("xtask-version-{}", std::process::id()));
    fs::create_dir_all(&root).expect("建得了");
    let manifest = "[workspace.package]\nversion = \"0.0.0\"\n";
    fs::write(root.join("Cargo.toml"), manifest).expect("写得进");
    let error = set(&root, "false", &["2026.01.0".to_string()]).expect_err("补零的不收");
    assert!(error.contains("不补零"), "{error}");
    assert!(set(&root, "false", &[]).expect_err("没写").contains("用法"));
    assert!(
        set(&root, "false", &["2026.11.0".to_string(), "x".to_string()])
            .expect_err("多写")
            .contains("用法")
    );
    let left = fs::read_to_string(root.join("Cargo.toml")).expect("读得到");
    fs::remove_dir_all(&root).expect("删得掉");
    assert_eq!(left, manifest);
}
