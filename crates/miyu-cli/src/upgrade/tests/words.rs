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
        Said::UnknownSystem,
        Said::Net(NetError::Offline("dns".to_string())),
        Said::Net(NetError::Limited(Some(1_790_000_000))),
        Said::Net(NetError::Limited(None)),
        Said::Net(NetError::Status(500)),
        Said::NoRelease,
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
        let limited = language.upgrade(&Said::Net(NetError::Limited(Some(1_790_000_000))));
        assert!(
            limited
                .iter()
                .any(|l| l.contains(':') && l.chars().any(|c| c.is_ascii_digit())),
            "{limited:?}"
        );
        assert!(
            language
                .upgrade(&Said::Upgraded {
                    from: "0.9.0".into(),
                    to: "1.0.0".into(),
                    left: None
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
