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

/// 造一份 `SHA256SUMS`：核心 `0.0.0-rc.5`，终端 `0.1.0`，网页 `0.2.0`；四个一体的加九份分包。哈希和名字之间一个空格（二进制
/// 模式带 `*`），`sha256sum` 两种都写。
fn sums() -> String {
    let mut lines = Vec::new();
    for (index, (target, _)) in TARGETS.iter().enumerate() {
        for (part, version, digit) in [
            ("miyu-core", "0.0.0-rc.5", 'a'),
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
        lines.push(format!("{}  miyu-0.0.0-rc.5-{target}.tar.gz", hash('0')));
    }
    lines.push(format!("{}  miyu-0.0.0-rc.5-windows-x86_64.zip", hash('1')));
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
        "v0.0.0-rc.5".to_string(),
        "--sums".to_string(),
        sums.to_string_lossy().into_owned(),
        "--out".to_string(),
        scratch.0.join("out").to_string_lossy().into_owned(),
    ];
    packaging(&root(), None, &args)
}

fn out(scratch: &Scratch, file: &str) -> String {
    fs::read_to_string(scratch.0.join("out").join(file)).expect("填出来了")
}

#[test]
fn every_template_is_filled_with_its_own_version_and_hashes() {
    let scratch = Scratch::new("good");
    let written = filled(&scratch, &sums()).expect("填得全");
    assert_eq!(written.len(), FILES.len() + COPIED.len());

    let core = out(&scratch, "aur/miyu-core/PKGBUILD");
    assert!(
        core.contains("_version=0.0.0-rc.5\npkgver=0.0.0rc.5\n"),
        "{core}"
    );
    assert!(core.contains(&format!("sha256sums_x86_64=('{}')", hash('a'))));
    assert!(core.contains(&format!("sha256sums_aarch64=('{}')", hash('b'))));

    let tui = out(&scratch, "aur/miyu-tui/PKGBUILD");
    assert!(
        tui.contains("_release=0.0.0-rc.5\npkgver=0.1.0\n_version=0.1.0\n"),
        "{tui}"
    );
    assert!(tui.contains(&format!("sha256sums_x86_64=('{}')", hash('b'))));
    let web = out(&scratch, "aur/miyu-webui/PKGBUILD");
    assert!(web.contains("pkgver=0.2.0\n"), "{web}");
    assert!(web.contains(&format!("sha256sums_aarch64=('{}')", hash('d'))));

    let agent = out(&scratch, "aur/miyu-agent/PKGBUILD");
    assert!(agent.contains("pkgver=0.0.0rc.5\n"), "{agent}");

    let formula = out(&scratch, "homebrew/Formula/miyu-tui.rb");
    assert!(
        formula.contains("/download/v0.0.0-rc.5/miyu-tui-0.1.0-macos-aarch64.tar.gz"),
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
        "miyu-core-0.0.0-rc.5-linux-x86_64",
        "miyu-core-0.0.0-rc.4-linux-x86_64",
    );
    let error = filled(&scratch, &core_missing).expect_err("核心那份不是这一版");
    assert!(
        error.contains("miyu-core-0.0.0-rc.5-linux-x86_64.tar.gz"),
        "{error}"
    );
}

#[test]
fn bad_sums_lines_and_tags_are_refused() {
    let scratch = Scratch::new("bad");
    let error = filled(
        &scratch,
        "nothex  miyu-core-0.0.0-rc.5-linux-x86_64.tar.gz\n",
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
fn the_core_conflicts_with_the_old_miyu_and_nothing_replaces_it() {
    for (template, _, _) in FILES {
        let text = read(&root().join("packaging").join(template)).unwrap();
        assert!(
            !text.contains("replaces="),
            "{template} 不写 replaces（core1 11-05）"
        );
    }
    let core = read(&root().join("packaging/aur/miyu-core/PKGBUILD.in")).unwrap();
    assert!(core.contains("conflicts=('miyu')") && core.contains("provides=('miyu')"));
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
