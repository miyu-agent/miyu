//! 照发行版订阅挑（施工 UP-4）：只认 `releases/tag/v<版本>` 的标签，带 `-` 的是预发布、`--prerelease` 才算；文件地址按标签拼；
//! 没带文件的（`SHA256SUMS` 回 404）跳过、接着往旧的找，找到不比现在新的就停：只升不降。

use super::*;

fn pre() -> Upgrade {
    Upgrade {
        prerelease: true,
        yes: true,
    }
}

#[test]
fn the_newest_stable_release_is_picked_without_prerelease() {
    let mut setup = Setup::new("stable", "1.0.0");
    // 订阅里的先后不作数，照版本比；正文里转义过的链接不是发行版。
    setup.tags(&["v0.8.0", "v1.1.0-rc.1", "v1.0.0", "nightly"]);
    let (said, steps, asked) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
    assert!(
        matches!(said, Said::Upgraded { ref to, .. } if to == "1.0.0"),
        "{said:?}"
    );
    assert_eq!(asked, 1);
    assert_eq!(
        steps[0],
        Step::Downloading(format!("miyu-1.0.0-{TARGET}.tar.gz"))
    );
    assert!(!setup.asked_for("rc.1"), "没加 --prerelease 的不碰预发布");
    assert!(!setup.asked_for("v99.0.0"), "正文里的链接不算");
    assert!(!setup.asked_for("api.github.com"), "不走 API");
    let base = download("v1.0.0");
    assert_eq!(
        *setup.net.asked.borrow(),
        vec![
            FEED.to_string(),
            format!("{base}/SHA256SUMS"),
            format!("{base}/SHA256SUMS.sig"),
            format!("{base}/miyu-1.0.0-{TARGET}.tar.gz"),
        ],
        "文件地址按标签拼"
    );
}

#[test]
fn prerelease_picks_the_newest_rc() {
    let mut setup = Setup::new("rc", "1.0.0-rc.2");
    setup.tags(&["v1.0.0-rc.2", "v1.0.0-rc.1", "v0.8.0"]);
    let (said, _, _) = run(&setup, pre(), "0.9.0", "miyu 1.0.0-rc.2", Answer::No);
    assert!(
        matches!(said, Said::Upgraded { ref to, .. } if to == "1.0.0-rc.2"),
        "{said:?}"
    );
}

#[test]
fn a_release_without_files_is_skipped_for_the_next_older_one() {
    let mut setup = Setup::new("skip", "1.0.0");
    setup.tags(&["v1.1.0", "v1.0.0", "v0.8.0"]);
    let (said, steps, asked) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
    assert!(
        matches!(said, Said::Upgraded { ref to, .. } if to == "1.0.0"),
        "{said:?}"
    );
    assert_eq!(asked, 1, "问的是能用的那一版");
    assert_eq!(
        steps[0],
        Step::Downloading(format!("miyu-1.0.0-{TARGET}.tar.gz"))
    );
    assert!(setup.asked_for("v1.1.0/SHA256SUMS"));
    assert!(
        !setup.asked_for("v1.1.0/SHA256SUMS.sig"),
        "清单没有就不往下要"
    );

    // 作废的预发布（同 rc.3：订阅里有，没有包）照样跳过。
    let mut setup = Setup::new("voided", "1.0.0-rc.2");
    setup.tags(&["v1.0.0-rc.3", "v1.0.0-rc.2", "v1.0.0-rc.1"]);
    let (said, _, _) = run(&setup, pre(), "0.9.0", "miyu 1.0.0-rc.2", Answer::Yes);
    assert!(
        matches!(said, Said::Upgraded { ref to, .. } if to == "1.0.0-rc.2"),
        "{said:?}"
    );
}

#[test]
fn skipping_stops_at_the_current_version_and_never_goes_down() {
    let mut setup = Setup::new("floor", "1.0.0");
    setup.tags(&["v1.1.0", "v1.0.0", "v0.8.0"]);
    let (said, _, asked) = run(&setup, plain(), "1.0.0", "miyu 1.0.0", Answer::Yes);
    assert_eq!(said, Said::UpToDate("1.0.0".to_string()));
    assert_eq!(asked, 0);
    assert!(!setup.asked_for("v1.0.0/"), "不比现在新的不下");

    // 比现在新的都没带文件：没有能升的，照样是已是最新。
    setup.tags(&["v1.1.0", "v1.0.1"]);
    let (said, _, asked) = run(&setup, plain(), "1.0.0", "miyu 1.0.0", Answer::Yes);
    assert_eq!(said, Said::UpToDate("1.0.0".to_string()));
    assert_eq!(asked, 0);
    assert!(setup.asked_for("v1.0.1/SHA256SUMS"), "一个一个往旧的找");
    untouched(&setup);
}

#[test]
fn a_checksum_list_that_fails_otherwise_stops_instead_of_skipping() {
    let mut setup = Setup::new("sums500", "1.0.0");
    setup.tags(&["v1.1.0", "v1.0.0"]);
    setup.net.answers.insert(
        format!("{}/SHA256SUMS", download("v1.1.0")),
        Err(NetError::Status(500)),
    );
    let (said, _, asked) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
    assert_eq!(said, Said::Net(NetError::Status(500)));
    assert_eq!(asked, 0);
    assert!(!setup.asked_for("v1.0.0/"), "不当成没带文件往下找");
    untouched(&setup);
}
