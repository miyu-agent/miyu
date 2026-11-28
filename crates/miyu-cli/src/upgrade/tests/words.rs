//! 给人看的字：每一种结果两种语言都说得出，没成的照报错写。

use super::*;

#[test]
fn every_outcome_reads_in_both_languages() {
    use crate::language::Language;
    let all = [
        Said::PackageManager(Channel::Aur),
        Said::PackageManager(Channel::Brew),
        Said::Unmarked(flow::download_page()),
        Said::BadMarker(PathBuf::from("/x/install.toml"), "no channel".to_string()),
        Said::NoKeys,
        Said::UnknownSystem(flow::download_page()),
        Said::Net(NetError::Offline("dns".to_string())),
        Said::Net(NetError::Status(429)),
        Said::Net(NetError::Status(500)),
        Said::BadFeed("https://github.com/miyu-agent/miyu/releases.atom".to_string()),
        Said::NoRelease,
        Said::OnlyPre,
        Said::UpToDate("1.0.0".to_string()),
        Said::Cancelled,
        Said::Unconfirmed,
        Said::MissingAsset("SHA256SUMS".to_string()),
        Said::BadSignature("x".to_string()),
        Said::BadSums("x".to_string()),
        Said::HashMismatch("a.tar.gz".to_string()),
        Said::BadPackage(vec!["miyu-tui".to_string()]),
        Said::WrongVersion {
            expected: "miyu 1.0.0".to_string(),
            said: "miyu 0.9.0".to_string(),
        },
        Said::NotWritable(PathBuf::from("/usr/lib/miyu")),
        Said::Failed("move-old".to_string(), "busy".to_string()),
        Said::Upgraded {
            from: "0.9.0".to_string(),
            to: "1.0.0".to_string(),
            left: Some(PathBuf::from("/x/.miyu-old-0.9.0")),
            kept: vec!["notes.txt".to_string()],
        },
    ];
    for language in [Language::Chinese, Language::English] {
        for said in &all {
            let lines = language.upgrade(said);
            assert!(
                !lines.is_empty() && lines.iter().all(|line| !line.trim().is_empty()),
                "{said:?}"
            );
            if said.code() != 0 && !matches!(said, Said::Cancelled) {
                assert!(lines[0].starts_with("miyu: "), "没成的照报错写：{lines:?}");
            }
        }
        assert!(
            language
                .upgrade(&Said::PackageManager(Channel::Aur))
                .iter()
                .any(|l| l.contains("paru -Syu"))
        );
        assert!(
            language
                .upgrade(&Said::PackageManager(Channel::Brew))
                .iter()
                .any(|l| l.contains("brew upgrade"))
        );
        let status = language.upgrade(&Said::Net(NetError::Status(429)));
        assert!(
            status.iter().any(|l| l.contains("HTTP 429"))
                && !status.iter().any(|l| l.contains("60")),
            "照状态码说，不再提匿名额度：{status:?}"
        );
        assert!(
            language
                .upgrade(&Said::Upgraded {
                    from: "0.9.0".into(),
                    to: "1.0.0".into(),
                    left: None,
                    kept: Vec::new(),
                })
                .iter()
                .any(|l| l.contains("miyu core stop"))
        );
        assert!(
            language
                .upgrade_question("0.9.0", "1.0.0")
                .ends_with("[Y/n] ")
        );
        for step in [
            Step::Downloading("a.tar.gz".into()),
            Step::Checking,
            Step::Replacing,
        ] {
            assert!(!language.upgrade_step(&step).is_empty());
        }
    }
}

#[test]
fn what_was_left_in_the_install_folder_is_said_with_at_most_five_names() {
    use crate::language::Language;
    let kept: Vec<String> = ["a", "b", "c", "d", "e", "f", "g"]
        .iter()
        .map(|n| n.to_string())
        .collect();
    let said = |kept: Vec<String>, language: Language| {
        language.upgrade(&Said::Upgraded {
            from: "0.9.0".into(),
            to: "1.0.0".into(),
            left: None,
            kept,
        })
    };
    let zh = said(kept.clone(), Language::Chinese);
    let line = zh
        .iter()
        .find(|l| l.contains("不属于 Miyu 的内容"))
        .expect("说了");
    assert!(
        line.contains("还有 7 项")
            && line.contains("a、b、c、d、e")
            && !line.contains("f")
            && line.ends_with("……"),
        "{line}"
    );
    let en = said(vec!["notes.txt".into()], Language::English);
    let line = en.iter().find(|l| l.contains("not Miyu's")).expect("said");
    assert!(
        line.contains("1 item(s)") && line.ends_with("notes.txt"),
        "{line}"
    );
    assert!(
        !said(Vec::new(), Language::Chinese)
            .iter()
            .any(|l| l.contains("不属于 Miyu 的内容")),
        "没有的不说"
    );
}

/// 没有这种系统的发行包时说该怎么办（施工 WN-1）：发行包有哪几种，别的系统去发行页拿源码。
#[test]
fn an_unknown_system_is_told_what_to_do() {
    use crate::language::Language;
    let page = flow::download_page();
    for (language, hint) in [(Language::Chinese, "提示："), (Language::English, "hint: ")] {
        let lines = language.upgrade(&Said::UnknownSystem(page.clone()));
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(
            lines[1].starts_with(hint) && lines[1].contains(&page),
            "{lines:?}"
        );
    }
}
