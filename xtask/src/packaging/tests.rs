//! 照一份造出来的 `SHA256SUMS` 填仓库里真的模板：填得全、版本和哈希放对地方；缺了、对不上、读不懂的报错；守着 core1 定的
//! 「只 conflicts、不 replaces」和 `miyu-core` 写的文件类型同 `packaging/mime/miyu.xml`。

use super::*;

/// 仓库根目录。
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask 在仓库根下")
        .to_path_buf()
}

/// 一个哈希：64 个 `digit`。
fn hash(digit: char) -> String {
    std::iter::repeat_n(digit, 64).collect()
}

/// 造一份 `SHA256SUMS`：核心 `2026.11.0-rc.1`，终端 `0.1.0`，网页 `0.2.0`；四个一体的加九份分包。哈希和名字之间一个空格（二进制
/// 模式带 `*`），`sha256sum` 两种都写。
fn sums() -> String {
    let mut lines = Vec::new();
    for (index, (target, _)) in TARGETS.iter().enumerate() {
        for (part, version, digit) in [
            ("miyu-core", "2026.11.0-rc.1", 'a'),
            ("miyu-tui", "0.1.0", 'b'),
            ("miyu-webui", "0.2.0", 'c'),
        ] {
            let digit = char::from_digit(digit.to_digit(16).unwrap() + index as u32, 16).unwrap();
            // 二进制模式的写法（`*` 打头）也认：macOS 的终端那一行这样写。
            let star = if part == "miyu-tui" && *target == "macos-aarch64" {
                "*"
            } else {
                ""
            };
            lines.push(format!(
                "{} {star}{part}-{version}-{target}.tar.gz",
                hash(digit)
            ));
        }
        lines.push(format!(
            "{}  miyu-2026.11.0-rc.1-{target}.tar.gz",
            hash('0')
        ));
    }
    lines.push(format!(
        "{}  miyu-2026.11.0-rc.1-windows-x86_64.zip",
        hash('1')
    ));
    lines.join("\n") + "\n"
}

/// 临时目录，用完删。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("xtask-packaging-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("清得掉上次留下的");
        }
        fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}

/// 照 `text` 当 `SHA256SUMS` 跑一次，填进临时目录。
fn filled(scratch: &Scratch, text: &str) -> Result<Vec<PathBuf>, String> {
    let sums = scratch.0.join("SHA256SUMS");
    fs::write(&sums, text).expect("写得进");
    let args = [
        "v2026.11.0-rc.1".to_string(),
        "--sums".to_string(),
        sums.to_string_lossy().into_owned(),
        "--out".to_string(),
        scratch.0.join("out").to_string_lossy().into_owned(),
    ];
    packaging(&root(), None, &args).map(|(written, _)| written)
}

fn out(scratch: &Scratch, file: &str) -> String {
    fs::read_to_string(scratch.0.join("out").join(file)).expect("填出来了")
}

#[test]
fn every_template_is_filled_with_its_own_version_and_hashes() {
    let scratch = Scratch::new("good");
    let written = filled(&scratch, &sums()).expect("填得全");
    assert_eq!(
        written.len(),
        FILES.len() - 2 + COPIED.len(),
        "没写 --applet，托盘那两份不填"
    );
    assert!(!scratch.0.join("out/aur/miyu-applet").exists());

    let core = out(&scratch, "aur/miyu-core/PKGBUILD");
    assert!(
        core.contains("_version=2026.11.0-rc.1\npkgver=2026.11.0rc1\n"),
        "{core}"
    );
    assert!(core.contains(&format!("sha256sums_x86_64=('{}')", hash('a'))));
    assert!(core.contains(&format!("sha256sums_aarch64=('{}')", hash('b'))));

    let tui = out(&scratch, "aur/miyu-tui/PKGBUILD");
    assert!(
        tui.contains("_release=2026.11.0-rc.1\npkgver=0.1.0\n_version=0.1.0\n"),
        "{tui}"
    );
    assert!(tui.contains(&format!("sha256sums_x86_64=('{}')", hash('b'))));
    let web = out(&scratch, "aur/miyu-webui/PKGBUILD");
    assert!(web.contains("pkgver=0.2.0\n"), "{web}");
    assert!(web.contains(&format!("sha256sums_aarch64=('{}')", hash('d'))));

    let agent = out(&scratch, "aur/miyu-agent/PKGBUILD");
    assert!(agent.contains("pkgver=2026.11.0rc1\n"), "{agent}");
    // 旧版的过渡包：版本跟核心走，只依赖合集，装上时照原样拷的脚本说话（施工 RL-5补2）。
    let transition = out(&scratch, "aur/miyu/PKGBUILD");
    assert!(
        transition.contains("pkgname=miyu\npkgver=2026.11.0rc1\n"),
        "{transition}"
    );
    assert!(transition.contains("depends=('miyu-agent')\ninstall=miyu.install\n"));
    assert!(!transition.contains("source"), "过渡包不带文件");
    assert_eq!(
        out(&scratch, "aur/miyu/miyu.install"),
        read(&root().join("packaging/aur/miyu/miyu.install")).unwrap()
    );

    let formula = out(&scratch, "homebrew/Formula/miyu-tui.rb");
    assert!(
        formula.contains("/download/v2026.11.0-rc.1/miyu-tui-0.1.0-macos-aarch64.tar.gz"),
        "{formula}"
    );
    assert!(formula.contains(&format!("sha256 \"{}\"", hash('d'))));
    let bundle = out(&scratch, "homebrew/Formula/miyu-agent.rb");
    let own = hex(&Sha256::digest(sums().as_bytes()));
    assert!(
        bundle.contains(&format!("sha256 \"{own}\"")),
        "合集照 SHA256SUMS 自己的哈希"
    );

    assert_eq!(
        out(&scratch, "aur/miyu-agent-git/PKGBUILD"),
        read(&root().join("packaging/aur/miyu-agent-git/PKGBUILD")).unwrap(),
        "-git 照原样"
    );
    for file in written {
        let text = fs::read_to_string(&file).unwrap();
        assert!(fill(&text, &[]).is_ok(), "{} 填全了", file.display());
    }
}

#[test]
fn a_missing_or_mismatched_archive_stops_everything() {
    let scratch = Scratch::new("missing");
    let missing = sums().replace("miyu-webui-0.2.0-macos-aarch64", "other");
    let error = filled(&scratch, &missing).expect_err("缺了一份");
    assert!(
        error.contains("miyu-webui-<版本>-macos-aarch64.tar.gz"),
        "{error}"
    );
    assert!(!scratch.0.join("out").exists(), "哪一份都不写");

    let mismatched = sums().replace(
        "miyu-tui-0.1.0-linux-aarch64",
        "miyu-tui-0.1.1-linux-aarch64",
    );
    let error = filled(&scratch, &mismatched).expect_err("版本对不上");
    assert!(error.contains("三种系统写的版本不一样"), "{error}");

    let core_missing = sums().replace(
        "miyu-core-2026.11.0-rc.1-linux-x86_64",
        "miyu-core-2026.11.0-rc.0-linux-x86_64",
    );
    let error = filled(&scratch, &core_missing).expect_err("核心那份不是这一版");
    assert!(
        error.contains("miyu-core-2026.11.0-rc.1-linux-x86_64.tar.gz"),
        "{error}"
    );
}

#[test]
fn bad_sums_lines_and_tags_are_refused() {
    let scratch = Scratch::new("bad");
    let error = filled(
        &scratch,
        "nothex  miyu-core-2026.11.0-rc.1-linux-x86_64.tar.gz\n",
    )
    .expect_err("哈希不对");
    assert!(error.contains("哈希不对"), "{error}");
    let error = filled(&scratch, "justoneword\n").expect_err("读不懂");
    assert!(error.contains("读不懂"), "{error}");
    let sums = scratch.0.join("SHA256SUMS");
    let args = [
        "0.0.0".to_string(),
        "--sums".to_string(),
        sums.to_string_lossy().into_owned(),
    ];
    assert!(
        packaging(&root(), None, &args)
            .expect_err("标签没有 v")
            .contains("v<版本>")
    );
    let args = ["v1".to_string()];
    assert!(
        packaging(&root(), None, &args)
            .expect_err("没写 --sums")
            .contains("用法")
    );
}

#[test]
fn leftover_placeholders_are_named_and_emails_are_not() {
    let values = [("VERSION".to_string(), "1.0.0".to_string())];
    assert_eq!(fill("<a@b.c> @VERSION@", &values).unwrap(), "<a@b.c> 1.0.0");
    let error = fill(
        "# x <shorin@users.noreply.github.com>\nv=@VERSION@ s=@SHA256_SUMS@",
        &values,
    )
    .expect_err("剩一个");
    assert!(error.contains("SHA256_SUMS"), "{error}");
    assert!(fill("@@ and @lower@", &[]).is_ok());
}

#[test]
fn the_core_conflicts_with_the_old_miyu_only_and_nothing_replaces_it() {
    for (template, _, _) in FILES {
        let text = read(&root().join("packaging").join(template)).unwrap();
        assert!(
            !text.contains("replaces="),
            "{template} 不写 replaces（core1 11-05）"
        );
    }
    // 只挡 0.7.x 那一代、不冒充过渡包 `miyu`（施工 RL-5补2）：`-git` 也一样。
    let core = read(&root().join("packaging/aur/miyu-core/PKGBUILD.in")).unwrap();
    assert!(core.contains("\nconflicts=('miyu<2026')\n"), "{core}");
    assert!(!core.contains("\nprovides="), "{core}");
    let git = read(&root().join("packaging/aur/miyu-agent-git/PKGBUILD")).unwrap();
    let field = |name: &str| {
        git.lines()
            .find(|line| line.starts_with(&format!("{name}=(")))
            .unwrap_or_default()
            .to_string()
    };
    assert!(field("conflicts").contains("'miyu<2026'"), "{git}");
    assert!(!field("conflicts").contains("'miyu'"), "{git}");
    assert!(!field("provides").contains("'miyu'"), "{git}");
    assert!(field("provides").contains("'miyu-agent'"), "{git}");
    // 过渡包装上、升级时都说怎么搬旧数据。
    let install = read(&root().join("packaging/aur/miyu/miyu.install")).unwrap();
    assert!(install.contains("post_install()") && install.contains("post_upgrade()"));
    assert!(install.contains("miyu migrate"));
    assert!(
        core.lines().any(|line| line.contains("install.toml")
            && line.trim_start().starts_with("printf")
            && line.contains("channel = \"aur\"")),
        "写 install.toml 的那一行写 aur"
    );
}

/// 去掉注释、把空白压成一个空格。
fn normalized(xml: &str) -> String {
    let mut text = xml.to_string();
    while let (Some(start), Some(end)) = (text.find("<!--"), text.find("-->")) {
        text.replace_range(start..end + 3, "");
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn the_core_package_writes_the_same_file_type_as_the_repository() {
    let core = read(&root().join("packaging/aur/miyu-core/PKGBUILD.in")).unwrap();
    let start = core.find("<<'XML'\n").expect("写了文件类型") + "<<'XML'\n".len();
    let end = start + core[start..].find("\nXML\n").expect("XML 收尾");
    let mime = read(&root().join("packaging/mime/miyu.xml")).unwrap();
    assert_eq!(normalized(&core[start..end]), normalized(&mime));
}

/// 托盘的发布（施工 RL-5补）：`miyu-applet-<版本>-linux-<架构>.tar.gz`、`-macos-aarch64.zip`。
fn applet_sums() -> String {
    [
        format!("{}  miyu-applet-0.1.0-linux-x86_64.tar.gz", hash('7')),
        format!("{}  miyu-applet-0.1.0-linux-aarch64.tar.gz", hash('8')),
        format!("{}  miyu-applet-0.1.0-macos-aarch64.zip", hash('9')),
    ]
    .join("\n")
        + "\n"
}

/// 照核心、托盘两次发布一起填。
fn with_applet(scratch: &Scratch, applet: &str) -> Result<(Vec<PathBuf>, bool), String> {
    let core = scratch.0.join("SHA256SUMS");
    let own = scratch.0.join("APPLET-SHA256SUMS");
    fs::write(&core, sums()).expect("写得进");
    fs::write(&own, applet).expect("写得进");
    let args = [
        "v2026.11.0-rc.1".to_string(),
        "--sums".to_string(),
        core.to_string_lossy().into_owned(),
        "--applet".to_string(),
        "v0.1.0".to_string(),
        "--applet-sums".to_string(),
        own.to_string_lossy().into_owned(),
        "--out".to_string(),
        scratch.0.join("out").to_string_lossy().into_owned(),
    ];
    packaging(&root(), None, &args)
}

#[test]
fn the_applet_is_filled_from_its_own_release() {
    let scratch = Scratch::new("applet");
    let (written, applet) = with_applet(&scratch, &applet_sums()).expect("填得全");
    assert!(applet);
    assert_eq!(written.len(), FILES.len() + COPIED.len());
    let aur = out(&scratch, "aur/miyu-applet/PKGBUILD");
    assert!(aur.contains("_version=0.1.0\npkgver=0.1.0\n"), "{aur}");
    assert!(aur.contains(&format!("sha256sums_x86_64=('{}')", hash('7'))));
    assert!(aur.contains(&format!("sha256sums_aarch64=('{}')", hash('8'))));
    let cask = out(&scratch, "homebrew/Casks/miyu-applet.rb");
    assert!(cask.contains("version \"0.1.0\""), "{cask}");
    assert!(
        cask.contains(&format!("sha256 \"{}\"", hash('9'))),
        "macOS 的是 zip"
    );
    let core = out(&scratch, "aur/miyu-core/PKGBUILD");
    assert!(
        core.contains("_version=2026.11.0-rc.1\n"),
        "核心照核心的发布"
    );
    let bundle = out(&scratch, "aur/miyu-agent/PKGBUILD");
    assert!(bundle.contains("'miyu-applet'"), "合集带托盘：{bundle}");
}

#[test]
fn an_applet_release_must_be_whole_and_both_options_go_together() {
    let scratch = Scratch::new("applet-bad");
    let missing = applet_sums().replace("macos-aarch64.zip", "macos-aarch64.tar.gz");
    let error = with_applet(&scratch, &missing).expect_err("macOS 的要是 zip");
    assert!(
        error.contains("miyu-applet-0.1.0-macos-aarch64.zip"),
        "{error}"
    );
    assert!(!scratch.0.join("out").exists(), "哪一份都不写");
    let sums = scratch.0.join("SHA256SUMS");
    let args = [
        "v2026.11.0-rc.1".to_string(),
        "--sums".to_string(),
        sums.to_string_lossy().into_owned(),
        "--applet".to_string(),
        "v0.1.0".to_string(),
    ];
    let error = packaging(&root(), None, &args).expect_err("只写了 --applet");
    assert!(error.contains("要一起写"), "{error}");
}

/// 没写 `--applet` 的：托盘那两份不填，交回「没填托盘」，`run` 照它说一句。
#[test]
fn without_an_applet_release_the_applet_is_reported_as_not_filled() {
    let scratch = Scratch::new("no-applet");
    let file = scratch.0.join("SHA256SUMS");
    fs::write(&file, sums()).expect("写得进");
    let args = [
        "v2026.11.0-rc.1".to_string(),
        "--sums".to_string(),
        file.to_string_lossy().into_owned(),
        "--out".to_string(),
        scratch.0.join("out").to_string_lossy().into_owned(),
    ];
    let (_, applet) = packaging(&root(), None, &args).expect("填得全");
    assert!(!applet);
    let (_, applet) = with_applet(&scratch, &applet_sums()).expect("填得全");
    assert!(applet);
}

/// pacman 的 pkgver（施工 VR-1）：预发布写成紧跟的 `rc<号>`，没有 `-`。
#[test]
fn a_pkgver_has_no_dash_and_a_tight_rc() {
    assert_eq!(pkgver("2026.11.0"), "2026.11.0");
    assert_eq!(pkgver("2026.11.0-rc.1"), "2026.11.0rc1");
    assert_eq!(pkgver("2026.12.3-rc.10"), "2026.12.3rc10");
    assert_eq!(pkgver("0.1.0-beta"), "0.1.0beta");
}
