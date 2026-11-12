//! 分包的名字、版本、搭出来的样子。

use std::fs;

use super::*;

/// 测试的临时目录，用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("xtask-split-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("清得掉上次留下的");
        }
        fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }

    fn put(&self, file: &str, text: &str) -> PathBuf {
        let path = self.0.join(file);
        fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
        fs::write(&path, text).expect("写得进");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}

#[test]
fn only_windows_goes_without() {
    assert!(wanted("linux-x86_64") && wanted("linux-aarch64") && wanted("macos-aarch64"));
    assert!(!wanted("windows-x86_64"));
}

#[test]
fn names_carry_their_own_versions() {
    assert_eq!(
        core_name("1.0.0", "linux-x86_64"),
        "miyu-core-1.0.0-linux-x86_64"
    );
    assert_eq!(
        core_name("2026.11.0-rc.1", "macos-aarch64"),
        "miyu-core-2026.11.0-rc.1-macos-aarch64"
    );
    assert_eq!(
        head_name("tui", "0.1.0", "macos-aarch64"),
        "tui-0.1.0-macos-aarch64"
    );
    assert_eq!(
        version_of("[package]\nversion = \"0.1.0\"\nname = { en = \"T\" }\n"),
        Ok("0.1.0".to_string())
    );
    assert!(
        version_of("[package]\nname = { en = \"T\" }\n")
            .unwrap_err()
            .contains("没有 version")
    );
    assert!(version_of("[[[").unwrap_err().contains("不是 TOML"));
}

#[test]
fn the_core_one_has_three_programs_and_only_the_core_resources() {
    let scratch = Scratch::new("core");
    let programs: Vec<PathBuf> = ["miyu", "miyu-sandbox", "miyu-extract"]
        .iter()
        .map(|name| scratch.put(&format!("build/{name}"), name))
        .collect();
    scratch.put("res/core/a.md", "a");
    scratch.put("res/packages/base-x/package.toml", "x");
    let files = vec![
        PathBuf::from("core/a.md"),
        PathBuf::from("packages/base-x/package.toml"),
    ];
    let license = scratch.put("LICENSE", "agpl");
    let dir = scratch.0.join("out/miyu-core-1.0.0-t");
    stage_core(
        &programs,
        (&scratch.0.join("res"), &files),
        &license,
        "core notices\n",
        &dir,
    )
    .unwrap();
    for name in ["miyu", "miyu-sandbox", "miyu-extract"] {
        assert_eq!(fs::read_to_string(dir.join(name)).unwrap(), name);
    }
    assert!(!dir.join("miyu-tui").exists() && !dir.join("README.txt").exists());
    assert_eq!(
        fs::read_to_string(dir.join("resources/core/a.md")).unwrap(),
        "a"
    );
    assert!(dir.join("resources/packages/base-x/package.toml").is_file());
    assert_eq!(
        fs::read_to_string(dir.join("THIRD-PARTY-LICENSES.txt")).unwrap(),
        "core notices\n"
    );
    assert_eq!(fs::read_to_string(dir.join("LICENSE")).unwrap(), "agpl");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(dir.join("miyu")).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
    assert!(
        stage_core(
            &programs,
            (&scratch.0.join("res"), &files),
            &license,
            "",
            &dir
        )
        .unwrap_err()
        .contains("已经有了")
    );
}

#[test]
fn a_head_folder_keeps_its_manifest_and_puts_the_program_in_bin() {
    let scratch = Scratch::new("head");
    let manifest = scratch.put("tui/tui/package.toml", "[package]\nprefix = \"tui\"\n");
    let license = scratch.put("tui/LICENSE", "agpl");
    let program = scratch.put("build/miyu-tui", "program");
    let dir = scratch.0.join("split/tui/tui");
    let files = vec![
        (manifest, PathBuf::from("package.toml")),
        (license, PathBuf::from("LICENSE")),
    ];
    stage_head(&files, &program, &dir).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("package.toml")).unwrap(),
        "[package]\nprefix = \"tui\"\n",
        "清单原样"
    );
    assert_eq!(
        fs::read_to_string(dir.join("bin/miyu-tui")).unwrap(),
        "program"
    );
    assert!(dir.join("LICENSE").is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(dir.join("bin/miyu-tui"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }
}
