//! 安装脚本（施工 RL-3，`packaging/install.sh`、`install.ps1`）里抄的两样和真相源一样：发版公钥照 [`miyu_source::sign::RELEASE`]
//! 的最后一把（换了钥匙忘了改脚本的会红），`install.sh` 写的 `.miyu` 文件类型和 `packaging/mime/miyu.xml` 挂的类型、glob 一样；
//! 两份脚本挑版本读发行版订阅、不问 GitHub API（施工 UP-5）。

use std::path::{Path, PathBuf};

fn packaging(file: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}：{error}", path.display()))
}

#[test]
fn both_scripts_carry_the_current_release_key() {
    let (id, key) = miyu_source::sign::RELEASE.last().expect("有发版公钥");
    let sh = packaging("install.sh");
    assert!(
        sh.contains(&format!("KEY_ID=\"{id}\"")),
        "install.sh 的编号"
    );
    assert!(sh.contains(&format!("KEY=\"{key}\"")), "install.sh 的公钥");
    let ps1 = packaging("install.ps1");
    assert!(
        ps1.contains(&format!("$KeyId = '{id}'")),
        "install.ps1 的编号"
    );
    assert!(
        ps1.contains(&format!("$Key = '{key}'")),
        "install.ps1 的公钥"
    );
}

/// 一份 shared-mime-info 定义挂的（类型, [(glob, 权重)…]）。
fn mime_types(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let document = roxmltree::Document::parse(text).expect("是合写法的 XML");
    document
        .root_element()
        .children()
        .filter(|node| node.is_element())
        .map(|kind| {
            let globs = kind
                .children()
                .filter(|node| node.is_element())
                .map(|glob| {
                    (
                        glob.attribute("pattern").unwrap_or_default().to_string(),
                        glob.attribute("weight").unwrap_or_default().to_string(),
                    )
                })
                .collect();
            (
                kind.attribute("type").unwrap_or_default().to_string(),
                globs,
            )
        })
        .collect()
}

#[test]
fn install_sh_writes_the_same_file_type_as_the_repository() {
    let sh = packaging("install.sh");
    let start = sh.find("MIME_XML='").expect("install.sh 写了文件类型") + "MIME_XML='".len();
    let length = sh[start..].find('\'').expect("单引号收尾");
    let written = &sh[start..start + length];
    assert_eq!(mime_types(written), mime_types(&packaging("mime/miyu.xml")));
}

/// `install.ps1` 不带 BOM、注释以外只有 ASCII：Windows PowerShell 5.1 用 `-File` 跑不带 BOM 的脚本照系统代码页解，字符串里的中文会
/// 读出弯引号把字符串截断；带了 BOM，`irm` 下载再建脚本块又认不出开头的注释块（施工 RL-3 的两趟 CI）。中文提示写成 `\uXXXX`。
#[test]
fn install_ps1_is_plain_ascii_outside_comments() {
    let ps1 = packaging("install.ps1");
    assert!(!ps1.starts_with('\u{feff}'), "不带 BOM");
    let mut in_block = false;
    for (number, line) in ps1.lines().enumerate() {
        let trimmed = line.trim();
        if in_block {
            in_block = !trimmed.contains("#>");
            continue;
        }
        if trimmed.starts_with("<#") {
            in_block = !trimmed.contains("#>");
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        assert!(line.is_ascii(), "第 {} 行不是 ASCII：{line}", number + 1);
    }
}

/// 两份安装脚本挑版本都读发行版订阅，不问 GitHub API（施工 UP-5，同 `miyu upgrade` 的 UP-4）：API 匿名每个 IP 每小时只给 60 次，
/// 共用出口的机器会撞上限流；订阅是普通网页，不算这个额度。
#[test]
fn both_scripts_read_the_release_feed_not_the_api() {
    for file in ["install.sh", "install.ps1"] {
        let script = packaging(file);
        assert!(
            !script.contains("api.github.com"),
            "{file} 还在问 GitHub API"
        );
        assert!(
            script.contains("https://github.com/${REPO}/releases.atom")
                || script.contains("https://github.com/$Repo/releases.atom"),
            "{file} 没读发行版订阅"
        );
    }
}
