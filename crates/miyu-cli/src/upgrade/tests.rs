//! `miyu upgrade` 走一趟：假的网络（发布列表、清单、签名、压缩包都是真造的）、真的钥匙、真的临时目录。每一种停下的地方都不碰
//! 装着的那一份。

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use miyu_source::sign;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::flow::{self, World};
use super::net::{Net, NetError};
use super::plan::{Channel, Version};
use super::swap::tests::{Scratch, new_version, tar_gz};
use super::{Answer, Said, Step, Upgrade};

const TARGET: &str = "linux-x86_64";

/// 照地址交回准备好的东西，记下问过哪些。
struct Fake {
    answers: HashMap<String, Result<Vec<u8>, NetError>>,
    asked: RefCell<Vec<String>>,
}

impl Net for Fake {
    fn get(&self, url: &str, _limit: usize, _api: bool) -> Result<Vec<u8>, NetError> {
        self.asked.borrow_mut().push(url.to_string());
        self.answers
            .get(url)
            .cloned()
            .unwrap_or(Err(NetError::Status(404)))
    }
}

/// 发布列表的地址（同 `flow`）。
fn list_url() -> String {
    format!(
        "https://api.github.com/repos/{}/releases?per_page=30",
        flow::REPOSITORY
    )
}

/// 一套：装着 0.9.0 的目录、一把钥匙、发布了 `version` 的网络。
struct Setup {
    scratch: Scratch,
    dir: PathBuf,
    keys: Vec<sign::PublicKey>,
    net: Fake,
}

impl Setup {
    fn new(name: &str, version: &str, prerelease: bool) -> Setup {
        let scratch = Scratch::new(&format!("flow-{name}"));
        scratch.put("install/miyu", "old");
        scratch.put("install/resources/core/a.md", "old resource");
        scratch.put("install/install.toml", "channel = \"script\"\n");
        let dir = scratch.0.join("install");
        let made = sign::generate().unwrap();
        let keys = vec![sign::PublicKey::from_base64(&made.public).unwrap()];
        let archive = format!("miyu-{version}-{TARGET}.tar.gz");
        let top = new_version(&scratch, &format!("build/miyu-{version}-{TARGET}"));
        let bytes = tar_gz(&top, &format!("miyu-{version}-{TARGET}"));
        let hash: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let sums = format!(
            "{hash}  {archive}\n{}  miyu-{version}-windows-x86_64.zip\n",
            "0".repeat(64)
        );
        let signature =
            serde_json::to_vec(&sign::sign(sums.as_bytes(), &made.secret).unwrap()).unwrap();
        let base = format!("https://github.com/x/releases/download/v{version}");
        let list = json!([
            {
                "tag_name": format!("v{version}"),
                "draft": false,
                "prerelease": prerelease,
                "assets": [
                    {"name": archive, "browser_download_url": format!("{base}/{archive}")},
                    {"name": "SHA256SUMS", "browser_download_url": format!("{base}/SHA256SUMS")},
                    {"name": "SHA256SUMS.sig", "browser_download_url": format!("{base}/SHA256SUMS.sig")},
                ],
            },
            {"tag_name": "v0.8.0", "draft": false, "prerelease": false, "assets": []},
        ]);
        let answers = HashMap::from([
            (list_url(), Ok(serde_json::to_vec(&list).unwrap())),
            (format!("{base}/{archive}"), Ok(bytes)),
            (format!("{base}/SHA256SUMS"), Ok(sums.into_bytes())),
            (format!("{base}/SHA256SUMS.sig"), Ok(signature)),
        ]);
        Setup {
            scratch,
            dir,
            keys,
            net: Fake {
                answers,
                asked: RefCell::new(Vec::new()),
            },
        }
    }

    /// 把 `url` 结尾的那一个换成 `answer`。
    fn answer(&mut self, ending: &str, answer: Result<Vec<u8>, NetError>) {
        let url = self
            .net
            .answers
            .keys()
            .find(|url| url.ends_with(ending))
            .cloned()
            .unwrap_or_else(|| panic!("没有 {ending}"));
        self.net.answers.insert(url, answer);
    }

    fn body(&self, ending: &str) -> Vec<u8> {
        self.net
            .answers
            .iter()
            .find(|(url, _)| url.ends_with(ending))
            .and_then(|(_, answer)| answer.clone().ok())
            .unwrap()
    }
}

/// 照 `setup` 走一趟：现在是 `current`，新版报 `reported`，问的时候答 `answer`。交回说的、走过的步、问过几次。
fn run(
    setup: &Setup,
    args: Upgrade,
    current: &str,
    reported: &str,
    answer: Answer,
) -> (Said, Vec<Step>, usize) {
    let mut steps = Vec::new();
    let mut asked = 0;
    let reported = reported.to_string();
    let version_of = move |_: &Path| Ok(reported.clone());
    let mut ask = |_: &Version, _: &Version| {
        asked += 1;
        answer
    };
    let mut step = |step: Step| steps.push(step);
    let mut rename = |from: &Path, to: &Path| fs::rename(from, to);
    let mut world = World {
        exe: setup.dir.join("miyu"),
        net: &setup.net,
        keys: setup.keys.clone(),
        target: Some(TARGET),
        exe_suffix: "",
        current: Version::parse(current).unwrap(),
        version_of: &version_of,
        ask: &mut ask,
        step: &mut step,
        rename: &mut rename,
    };
    let said = flow::run(&args, &mut world);
    drop(world);
    (said, steps, asked)
}

fn plain() -> Upgrade {
    Upgrade {
        prerelease: false,
        yes: false,
    }
}

/// 装着的那一份没动。
fn untouched(setup: &Setup) {
    assert_eq!(fs::read_to_string(setup.dir.join("miyu")).unwrap(), "old");
    assert!(!setup.dir.join("miyu-web").exists());
    assert!(!setup.dir.join(".miyu-upgrade").exists(), "暂存删了");
}

#[test]
fn a_script_install_upgrades_itself() {
    let setup = Setup::new("ok", "1.0.0", false);
    // 上一次删不掉、留下来的：这一次开头删（不删的话这一次的 .miyu-old-0.9.0 建不了）。
    setup.scratch.put("install/.miyu-old-0.9.0/miyu", "older");
    setup.scratch.put("install/.miyu-old-0.8.0/miyu", "oldest");
    // 人自己放进安装目录的：原样留着、说出来（施工 RL-4补3）。
    setup.scratch.put("install/notes.txt", "mine");
    let (said, steps, asked) = run(&setup, plain(), "0.9.0", "miyu 1.0.0\n", Answer::Yes);
    assert_eq!(
        said,
        Said::Upgraded {
            from: "0.9.0".to_string(),
            to: "1.0.0".to_string(),
            left: None,
            kept: vec!["notes.txt".to_string()],
        }
    );
    assert_eq!(
        fs::read_to_string(setup.dir.join("notes.txt")).unwrap(),
        "mine"
    );
    assert_eq!(asked, 1);
    assert_eq!(
        steps,
        vec![
            Step::Downloading(format!("miyu-1.0.0-{TARGET}.tar.gz")),
            Step::Checking,
            Step::Replacing
        ]
    );
    assert_eq!(fs::read_to_string(setup.dir.join("miyu")).unwrap(), "new");
    assert_eq!(
        fs::read_to_string(setup.dir.join("miyu-web")).unwrap(),
        "new"
    );
    assert_eq!(
        fs::read_to_string(setup.dir.join("resources/core/a.md")).unwrap(),
        "new resource"
    );
    assert_eq!(
        fs::read_to_string(setup.dir.join("install.toml")).unwrap(),
        "channel = \"script\"\n",
        "标记留着"
    );
    assert!(
        !setup.dir.join(".miyu-upgrade").exists() && !setup.dir.join(".miyu-old-0.9.0").exists()
    );
    assert!(
        !setup.dir.join(".miyu-old-0.8.0").exists(),
        "上次留下的删了"
    );
    assert!(setup.scratch.0.join("build").is_dir(), "只动安装目录");
}

#[test]
fn package_managers_and_unmarked_installs_are_only_told_what_to_do() {
    for (marker, said) in [
        (
            Some("channel = \"aur\"\n"),
            Said::PackageManager(Channel::Aur),
        ),
        (
            Some("channel = \"brew\"\n"),
            Said::PackageManager(Channel::Brew),
        ),
        (None, Said::Unmarked(flow::download_page())),
    ] {
        let setup = Setup::new("manager", "1.0.0", false);
        match marker {
            Some(text) => drop(setup.scratch.put("install/install.toml", text)),
            None => fs::remove_file(setup.dir.join("install.toml")).unwrap(),
        }
        let (got, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
        assert_eq!(got, said);
        assert_eq!(got.code(), 0);
        assert!(setup.net.asked.borrow().is_empty(), "不上网");
        untouched(&setup);
    }
}

#[test]
fn a_bad_marker_or_no_keys_or_an_unknown_system_stops_before_the_network() {
    let setup = Setup::new("marker", "1.0.0", false);
    setup
        .scratch
        .put("install/install.toml", "channel = \"apt\"\n");
    let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
    assert!(
        matches!(said, Said::BadMarker(_, ref why) if why.contains("apt")),
        "{said:?}"
    );

    let mut setup = Setup::new("nokeys", "1.0.0", false);
    setup.keys.clear();
    let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
    assert_eq!(said, Said::NoKeys);
    assert!(setup.net.asked.borrow().is_empty());
    untouched(&setup);
}

#[test]
fn network_trouble_is_named() {
    for (answer, said) in [
        (
            Err(NetError::Offline("dns".to_string())),
            Said::Net(NetError::Offline("dns".to_string())),
        ),
        (
            Err(NetError::Limited(Some(1_790_000_000))),
            Said::Net(NetError::Limited(Some(1_790_000_000))),
        ),
        (Err(NetError::Status(500)), Said::Net(NetError::Status(500))),
        (
            Ok(b"<html>".to_vec()),
            Said::Failed("releases".to_string(), "not JSON".to_string()),
        ),
    ] {
        let mut setup = Setup::new("net", "1.0.0", false);
        setup.answer("per_page=30", answer);
        let (got, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
        assert_eq!(got, said);
        assert_eq!(got.code(), 1);
        untouched(&setup);
    }
}

#[test]
fn nothing_newer_means_up_to_date_and_prereleases_need_pre() {
    let setup = Setup::new("same", "1.0.0", false);
    let (said, _, asked) = run(&setup, plain(), "1.0.0", "miyu 1.0.0", Answer::Yes);
    assert_eq!(said, Said::UpToDate("1.0.0".to_string()));
    assert_eq!(asked, 0);
    let (said, _, _) = run(&setup, plain(), "1.1.0", "miyu 1.0.0", Answer::Yes);
    assert_eq!(said, Said::UpToDate("1.1.0".to_string()), "不降级");

    let setup = Setup::new("pre", "1.0.0-rc.1", true);
    let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0-rc.1", Answer::Yes);
    assert_eq!(
        said,
        Said::UpToDate("0.9.0".to_string()),
        "预发布不算，剩下的 0.8.0 不比现在新"
    );
    let (said, _, _) = run(
        &setup,
        Upgrade {
            prerelease: true,
            yes: true,
        },
        "0.9.0",
        "miyu 1.0.0-rc.1",
        Answer::No,
    );
    assert!(
        matches!(said, Said::Upgraded { ref to, .. } if to == "1.0.0-rc.1"),
        "{said:?}"
    );
}

#[test]
fn the_answer_decides_and_yes_skips_the_question() {
    let setup = Setup::new("answer", "1.0.0", false);
    let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::No);
    assert_eq!(said, Said::Cancelled);
    let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Closed);
    assert_eq!(said, Said::Unconfirmed);
    untouched(&setup);
    let (said, _, asked) = run(
        &setup,
        Upgrade {
            prerelease: false,
            yes: true,
        },
        "0.9.0",
        "miyu 1.0.0",
        Answer::No,
    );
    assert!(matches!(said, Said::Upgraded { .. }), "{said:?}");
    assert_eq!(asked, 0);
}

mod refused;
mod words;
