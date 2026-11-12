//! 打包里不编、不取的那几步：读命令行、版本、名字、合许可证、README、搭目录、打 tar.gz 和 zip。

use std::io::Read;

use super::*;

/// 测试的临时目录，用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("xtask-dist-{name}-{}", std::process::id()));
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

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

fn pin(id: &str, rev: &str) -> Pin {
    Pin {
        id: id.to_string(),
        repo: format!("https://github.com/miyu-agent/miyu-{id}"),
        rev: rev.to_string(),
    }
}

#[test]
fn the_target_is_this_machine_unless_written_the_same() {
    assert_eq!(
        options(&[], Some("linux-x86_64")),
        Ok(Options {
            target: "linux-x86_64".to_string(),
            out: None
        })
    );
    assert_eq!(
        options(
            &words(&["--target", "linux-x86_64", "--out", "d"]),
            Some("linux-x86_64")
        ),
        Ok(Options {
            target: "linux-x86_64".to_string(),
            out: Some(PathBuf::from("d"))
        })
    );
    let other = options(
        &words(&["--target", "windows-x86_64"]),
        Some("linux-x86_64"),
    )
    .unwrap_err();
    assert!(other.contains("只打本机那一种"), "{other}");
    assert!(options(&words(&["--target"]), Some("linux-x86_64")).is_err());
    assert!(options(&words(&["--out"]), Some("linux-x86_64")).is_err());
    assert!(
        options(&words(&["--zip"]), Some("linux-x86_64"))
            .unwrap_err()
            .contains("不认识的参数")
    );
    assert!(options(&[], None).unwrap_err().contains("四种系统"));
}

#[test]
fn the_host_is_one_of_the_four_where_ci_runs() {
    if cfg!(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64")
    )) {
        assert!(HOST.is_some());
    }
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        assert_eq!(HOST, Some("linux-x86_64"));
    }
}

#[test]
fn the_version_is_the_workspace_one() {
    assert_eq!(
        version(
            "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"1.0.0\"\nedition = \"2024\"\n"
        ),
        Ok("1.0.0".to_string())
    );
    assert!(
        version("[workspace.package]\nedition = \"2024\"\n")
            .unwrap_err()
            .contains("没有 version")
    );
    assert!(version("[package]\nversion = \"1.0.0\"\n").is_err());
    assert!(version("not toml [").unwrap_err().contains("不是 TOML"));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    assert!(
        version(&read(&root.join("Cargo.toml")).unwrap()).is_ok(),
        "仓库里的读得出"
    );
}

#[test]
fn windows_gets_a_zip_and_the_rest_tar_gz() {
    assert_eq!(
        archive_name("1.0.0", "linux-x86_64"),
        "miyu-1.0.0-linux-x86_64.tar.gz"
    );
    assert_eq!(
        archive_name("1.0.0", "macos-aarch64"),
        "miyu-1.0.0-macos-aarch64.tar.gz"
    );
    assert_eq!(
        archive_name("1.0.0", "windows-x86_64"),
        "miyu-1.0.0-windows-x86_64.zip"
    );
    // 年.月.序号、演练的候选版（施工 VR-2）：版本原样夹在中间。
    assert_eq!(
        archive_name("2026.11.0-rc.1", "linux-x86_64"),
        "miyu-2026.11.0-rc.1-linux-x86_64.tar.gz"
    );
    assert_eq!(
        archive_name("2026.11.0", "windows-x86_64"),
        "miyu-2026.11.0-windows-x86_64.zip"
    );
}

#[test]
fn the_notices_join_core_then_the_two_heads_and_say_which_revision() {
    let text = notices(
        "CORE LIST\n\n",
        (
            &pin("tui", "46f2699c6ea326489e819292da64426ea90c858f"),
            "TUI LIST\n",
        ),
        (&pin("webui", "v0.1.0"), "WEB LIST"),
    );
    let core = text.find("CORE LIST").unwrap();
    let tui = text.find("TUI LIST").unwrap();
    let web = text.find("WEB LIST").unwrap();
    assert!(core < tui && tui < web, "{text}");
    assert!(text.contains(
        "https://github.com/miyu-agent/miyu-tui @ 46f2699c6ea326489e819292da64426ea90c858f"
    ));
    assert!(text.contains("https://github.com/miyu-agent/miyu-webui @ v0.1.0"));
    assert!(
        text.contains("CORE LIST\n\n===="),
        "末尾的空行收成一个：{text}"
    );
    assert!(text.ends_with("WEB LIST\n"));
}

#[test]
fn the_readme_has_both_languages_and_fits_the_system() {
    let linux = readme("1.0.0", "linux-x86_64");
    assert!(linux.starts_with("Miyu 1.0.0 (linux-x86_64)\n"));
    assert!(linux.contains("\n中文\n") && linux.contains("\nEnglish\n"));
    assert!(linux.contains("miyu service disable"));
    assert!(!linux.contains("miyu.exe"));
    let windows = readme("1.0.0", "windows-x86_64");
    assert!(windows.contains("miyu.exe"));
}

/// 一份搭目录要的东西：两个程序、两份登记了的资源（另有一份没登记的不带）、终端两样、网页一个带 `bin/` 的文件夹。
fn sources(scratch: &Scratch) -> Sources {
    let miyu = scratch.put("build/miyu", "core program");
    let tui = scratch.put("build/miyu-tui", "tui program");
    scratch.put("res/core/a.md", "a");
    scratch.put("res/packages/base-x/package.toml", "x");
    scratch.put("res/stray.txt", "not tracked");
    let manifest = scratch.put("tui/tui/package.toml", "tui manifest");
    let tui_license = scratch.put("tui/LICENSE", "tui license");
    scratch.put("web/package.toml", "web manifest");
    scratch.put("web/pages/index.html", "<html>");
    scratch.put("web/bin/miyu-web", "web program in bin");
    Sources {
        programs: vec![miyu, tui],
        resources: scratch.0.join("res"),
        resource_files: vec![
            PathBuf::from("core/a.md"),
            PathBuf::from("packages/base-x/package.toml"),
        ],
        tui_package: vec![
            (manifest, PathBuf::from("package.toml")),
            (tui_license, PathBuf::from("LICENSE")),
        ],
        web_package: scratch.0.join("web"),
        license: scratch.put("LICENSE", "agpl"),
        notices: "joined notices\n".to_string(),
        readme: "readme\n".to_string(),
    }
}

#[test]
fn staging_lays_out_programs_resources_heads_and_licenses() {
    let scratch = Scratch::new("stage");
    let sources = sources(&scratch);
    let dir = scratch.0.join("out/miyu-1.0.0-linux-x86_64");
    stage(&sources, &dir).unwrap();
    let text = |file: &str| fs::read_to_string(dir.join(file)).unwrap();
    assert_eq!(text("miyu"), "core program");
    assert_eq!(text("miyu-tui"), "tui program");
    assert_eq!(text("resources/core/a.md"), "a");
    assert_eq!(text("resources/packages/base-x/package.toml"), "x");
    assert!(!dir.join("resources/stray.txt").exists(), "没登记的不带");
    assert_eq!(text("resources/packages/tui/package.toml"), "tui manifest");
    assert_eq!(text("resources/packages/tui/LICENSE"), "tui license");
    assert_eq!(
        text("resources/packages/webui/package.toml"),
        "web manifest"
    );
    assert_eq!(text("resources/packages/webui/pages/index.html"), "<html>");
    assert!(
        !dir.join("resources/packages/webui/bin").exists(),
        "网页的程序放在 miyu 旁边"
    );
    assert_eq!(text("LICENSE"), "agpl");
    assert_eq!(text("THIRD-PARTY-LICENSES.txt"), "joined notices\n");
    assert_eq!(text("README.txt"), "readme\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bits = |file: &str| fs::metadata(dir.join(file)).unwrap().permissions().mode() & 0o777;
        assert_eq!(bits("miyu"), 0o755);
        assert_eq!(bits("miyu-tui"), 0o755);
    }
    assert!(
        stage(&sources, &dir).unwrap_err().contains("已经有了"),
        "不盖上一次的"
    );
}

#[test]
fn a_head_folder_already_in_the_core_resources_is_refused() {
    let scratch = Scratch::new("clash");
    let mut sources = sources(&scratch);
    scratch.put("res/packages/webui/package.toml", "old web");
    sources
        .resource_files
        .push(PathBuf::from("packages/webui/package.toml"));
    let said = stage(&sources, &scratch.0.join("out/x")).unwrap_err();
    assert!(said.contains("webui"), "{said}");
}

/// 搭一个小目录：一个能跑的、一个普通文件、一层子目录。
fn small(scratch: &Scratch) -> PathBuf {
    let dir = scratch.0.join("miyu-1.0.0-t");
    let program = scratch.put("miyu-1.0.0-t/miyu", "program");
    executable(&program).unwrap();
    scratch.put("miyu-1.0.0-t/resources/b.txt", "bee");
    scratch.put("miyu-1.0.0-t/README.txt", "readme");
    dir
}

#[test]
fn the_tar_gz_has_one_top_folder_sorted_with_modes_and_no_owner_or_time() {
    let scratch = Scratch::new("tar");
    let dir = small(&scratch);
    let archive = scratch.0.join("a.tar.gz");
    pack(&dir, "miyu-1.0.0-t", &archive).unwrap();
    let file = fs::File::open(&archive).unwrap();
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut seen = Vec::new();
    for entry in tar.entries().unwrap() {
        let mut entry = entry.unwrap();
        let header = entry.header().clone();
        let path = entry
            .path()
            .unwrap()
            .to_string_lossy()
            .trim_end_matches('/')
            .to_string();
        let mut body = String::new();
        entry.read_to_string(&mut body).unwrap();
        assert_eq!(header.mtime().unwrap(), 0);
        assert_eq!(header.uid().unwrap(), 0);
        seen.push((path, header.mode().unwrap(), body));
    }
    let expected = |path: &str, mode: u32, body: &str| (path.to_string(), mode, body.to_string());
    let program_mode = if cfg!(unix) { 0o755 } else { 0o644 };
    assert_eq!(
        seen,
        vec![
            expected("miyu-1.0.0-t", 0o755, ""),
            expected("miyu-1.0.0-t/README.txt", 0o644, "readme"),
            expected("miyu-1.0.0-t/miyu", program_mode, "program"),
            expected("miyu-1.0.0-t/resources", 0o755, ""),
            expected("miyu-1.0.0-t/resources/b.txt", 0o644, "bee"),
        ]
    );
    let again = scratch.0.join("b.tar.gz");
    pack(&dir, "miyu-1.0.0-t", &again).unwrap();
    assert_eq!(
        digest(&archive).unwrap(),
        digest(&again).unwrap(),
        "同样的目录打出来一字不差"
    );
}

#[test]
fn the_zip_has_the_same_layout() {
    let scratch = Scratch::new("zip");
    let dir = small(&scratch);
    let archive = scratch.0.join("a.zip");
    pack(&dir, "miyu-1.0.0-t", &archive).unwrap();
    let mut zip = zip::ZipArchive::new(fs::File::open(&archive).unwrap()).unwrap();
    let names: Vec<String> = zip
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        vec![
            "miyu-1.0.0-t/",
            "miyu-1.0.0-t/README.txt",
            "miyu-1.0.0-t/miyu",
            "miyu-1.0.0-t/resources/",
            "miyu-1.0.0-t/resources/b.txt",
        ]
    );
    let mut body = String::new();
    zip.by_name("miyu-1.0.0-t/resources/b.txt")
        .unwrap()
        .read_to_string(&mut body)
        .unwrap();
    assert_eq!(body, "bee");
}

#[test]
fn links_in_the_folder_are_refused() {
    #[cfg(unix)]
    {
        let scratch = Scratch::new("link");
        let dir = small(&scratch);
        std::os::unix::fs::symlink("README.txt", dir.join("link")).unwrap();
        let said = pack(&dir, "x", &scratch.0.join("a.tar.gz")).unwrap_err();
        assert!(said.contains("不是普通文件、目录"), "{said}");
    }
}

#[test]
fn the_digest_is_sha256_hex_and_the_size() {
    let scratch = Scratch::new("digest");
    let file = scratch.put("f", "abc");
    assert_eq!(
        digest(&file).unwrap(),
        (
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
            3
        )
    );
}

#[test]
fn head_builds_drop_the_outer_cargo_settings() {
    let command = head_cargo(Path::new("."));
    let removed: Vec<String> = command
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    // 测试本身由 cargo 跑，这几样一定在外面设着。
    for name in ["CARGO", "CARGO_MANIFEST_DIR", "CARGO_PKG_NAME"] {
        assert!(
            removed.iter().any(|one| one == name),
            "{name} 没去掉：{removed:?}"
        );
    }
    assert!(
        !removed
            .iter()
            .any(|one| one == "CARGO_HOME" || one == "PATH")
    );
}

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
