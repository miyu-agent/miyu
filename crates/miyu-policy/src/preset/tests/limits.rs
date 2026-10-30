//! `[limits]`（施工 AE-1）：读、逐格叠、写错写明第几行；写了才进指纹，没写的指纹和以前一字不差。

use super::*;

#[test]
fn unattended_steps_reads_and_stacks() {
    let file = read("[limits]\nunattended_steps = 300\n").unwrap();
    assert_eq!(file.unattended_steps, Some(300));
    assert_eq!(PresetFile::default().unattended_steps, None, "没写的没有");
    // 上一层写了的盖住下面的，没写的照下面的。
    let lower = read("[limits]\nunattended_steps = 300\n").unwrap();
    let upper = read("[limits]\nunattended_steps = 40\n").unwrap();
    assert_eq!(upper.over(lower.clone()).unattended_steps, Some(40));
    let silent = read("[preset]\nunlisted = \"off\"\n").unwrap();
    assert_eq!(silent.over(lower).unattended_steps, Some(300));
}

#[test]
fn a_bad_limit_names_its_line() {
    for value in ["0", "-3", "1.5", "\"500\"", "true", "4294967296"] {
        let problem = read(&format!(
            "[preset]\nunlisted = \"on\"\n\n[limits]\nunattended_steps = {value}\n"
        ))
        .expect_err(value);
        assert_eq!(
            (problem.code, problem.line, problem.detail.as_str()),
            (Code::BadLimit, Some(5), "limits.unattended_steps"),
            "{value}"
        );
    }
    let problem = read("[limits]\nsteps = 3\n").unwrap_err();
    assert_eq!(
        (problem.code, problem.line, problem.detail.as_str()),
        (Code::UnknownKey, Some(2), "limits.steps")
    );
    assert_eq!(wrong("limits = 3\n"), (Code::NotATable, Some(1)));
    assert_eq!(Code::BadLimit.as_str(), "bad_limit");
    assert!(
        Code::ALL.contains(&Code::BadLimit),
        "门禁照它查三种语言的字"
    );
}

/// 没写 `[limits]` 的指纹和以前一字不差（以前造的快照照旧对得上，开着的会话不白白换一次快照）：两条路（写没写 `[features]`）
/// 都钉住，值另用 Python 照 serde_json 的写法独立算过。写了的才变，写的数不一样指纹也不一样。
#[test]
fn the_digest_moves_only_when_limits_are_written() {
    let dev = read(DEV).unwrap();
    assert_eq!(
        dev.digest().as_str(),
        "sha256:89703aaa280c2505d05919571905728248c06efd0a9476326d15b7681e41d9dd"
    );
    let mut featured = dev.clone();
    featured.features.insert("files".to_string(), false);
    assert_eq!(
        featured.digest().as_str(),
        "sha256:53861c8ee879827379ccd7b017aaf605552490623d340914f00605e9053adba8"
    );
    let mut limited = dev.clone();
    limited.unattended_steps = Some(300);
    assert_ne!(limited.digest(), dev.digest());
    let mut other = dev;
    other.unattended_steps = Some(301);
    assert_ne!(other.digest(), limited.digest());
    featured.unattended_steps = Some(300);
    assert_ne!(featured.digest(), limited.digest(), "功能那一格照样算");
}
