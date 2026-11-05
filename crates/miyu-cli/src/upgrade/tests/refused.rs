//! 每一种停下的地方：验不过、清单不对、新版不全或报错版本、没写权限、对换中途出错、没有这种系统。都不碰装着的那一份。

use super::*;

#[test]
fn anything_that_does_not_verify_stops_and_leaves_the_install() {
    let other = sign::generate().unwrap();
    type Spoil = fn(&mut Setup, &sign::Generated);
    /// 一种弄坏法：名字、怎么弄坏、该停在哪。
    type Case = (&'static str, Spoil, fn(&Said) -> bool);
    let cases: [Case; 4] = [
        (
            "nosig",
            |s, _| {
                let list = s.body("per_page=30");
                let text = String::from_utf8(list)
                    .unwrap()
                    .replace("SHA256SUMS.sig", "other.sig");
                s.answer("per_page=30", Ok(text.into_bytes()));
            },
            |said| *said == Said::MissingAsset("SHA256SUMS.sig".to_string()),
        ),
        (
            "otherkey",
            |s, other| {
                let sums = s.body("/SHA256SUMS");
                let signed = sign::sign(&sums, &other.secret).unwrap();
                s.answer("SHA256SUMS.sig", Ok(serde_json::to_vec(&signed).unwrap()));
            },
            |said| matches!(said, Said::BadSignature(_)),
        ),
        (
            "tampered",
            |s, _| {
                let mut sums = s.body("/SHA256SUMS");
                sums[0] = if sums[0] == b'a' { b'b' } else { b'a' };
                s.answer("/SHA256SUMS", Ok(sums));
            },
            |said| matches!(said, Said::BadSignature(_)),
        ),
        (
            "archive",
            |s, _| {
                let mut bytes = s.body(".tar.gz");
                bytes.push(0);
                s.answer(".tar.gz", Ok(bytes));
            },
            |said| matches!(said, Said::HashMismatch(_)),
        ),
    ];
    for (name, spoil, expected) in cases {
        let mut setup = Setup::new(name, "1.0.0", false);
        spoil(&mut setup, &other);
        let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
        assert!(expected(&said), "{name}：{said:?}");
        assert_eq!(said.code(), 1);
        untouched(&setup);
    }
}

#[test]
fn a_signed_list_without_our_archive_or_badly_written_is_refused() {
    let made = sign::generate().unwrap();
    for (sums, expected) in [
        (
            format!("{}  miyu-1.0.0-macos-aarch64.tar.gz\n", "0".repeat(64)),
            Said::MissingAsset(format!("SHA256SUMS: miyu-1.0.0-{TARGET}.tar.gz")),
        ),
        (
            "oops\n".to_string(),
            Said::BadSums("line 1: not `<sha256>  <file>`".to_string()),
        ),
    ] {
        let mut setup = Setup::new("signedsums", "1.0.0", false);
        setup.keys = vec![sign::PublicKey::from_base64(&made.public).unwrap()];
        let signed = sign::sign(sums.as_bytes(), &made.secret).unwrap();
        setup.answer("/SHA256SUMS", Ok(sums.into_bytes()));
        setup.answer("SHA256SUMS.sig", Ok(serde_json::to_vec(&signed).unwrap()));
        let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
        assert_eq!(said, expected);
        untouched(&setup);
    }
}

#[test]
fn a_new_version_that_is_incomplete_or_says_another_version_is_not_put_in() {
    let setup = Setup::new("version", "1.0.0", false);
    let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 0.9.9", Answer::Yes);
    assert_eq!(
        said,
        Said::WrongVersion {
            expected: "miyu 1.0.0".to_string(),
            said: "miyu 0.9.9".to_string()
        }
    );
    untouched(&setup);

    let mut setup = Setup::new("incomplete", "1.0.0", false);
    let partial = new_version(&setup.scratch, &format!("partial/miyu-1.0.0-{TARGET}"));
    fs::remove_file(partial.join("miyu-tui")).unwrap();
    let bytes = tar_gz(&partial, &format!("miyu-1.0.0-{TARGET}"));
    let hash: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let made = sign::generate().unwrap();
    setup.keys = vec![sign::PublicKey::from_base64(&made.public).unwrap()];
    let sums = format!("{hash}  miyu-1.0.0-{TARGET}.tar.gz\n");
    let signed = sign::sign(sums.as_bytes(), &made.secret).unwrap();
    setup.answer(".tar.gz", Ok(bytes));
    setup.answer("/SHA256SUMS", Ok(sums.into_bytes()));
    setup.answer("SHA256SUMS.sig", Ok(serde_json::to_vec(&signed).unwrap()));
    let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
    assert_eq!(said, Said::BadPackage(vec!["miyu-tui".to_string()]));
    untouched(&setup);
}

#[cfg(unix)]
#[test]
fn an_install_it_cannot_write_is_said_and_not_forced() {
    use std::os::unix::fs::PermissionsExt;
    let setup = Setup::new("readonly", "1.0.0", false);
    fs::set_permissions(&setup.dir, fs::Permissions::from_mode(0o555)).unwrap();
    let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0", Answer::Yes);
    fs::set_permissions(&setup.dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(said, Said::NotWritable(setup.dir.clone()));
    untouched(&setup);
}

#[test]
fn a_failed_swap_keeps_the_installed_version() {
    let setup = Setup::new("swapfail", "1.0.0", false);
    let reported = "miyu 1.0.0".to_string();
    let version_of = move |_: &Path| Ok(reported.clone());
    let mut ask = |_: &Version, _: &Version| Answer::Yes;
    let mut step = |_: Step| {};
    let mut moved = 0;
    let mut rename = |from: &Path, to: &Path| {
        moved += 1;
        match moved {
            2 => Err(io::Error::other("busy")),
            _ => fs::rename(from, to),
        }
    };
    let mut world = World {
        exe: setup.dir.join("miyu"),
        net: &setup.net,
        keys: setup.keys.clone(),
        target: Some(TARGET),
        exe_suffix: "",
        current: Version::parse("0.9.0").unwrap(),
        version_of: &version_of,
        ask: &mut ask,
        step: &mut step,
        rename: &mut rename,
    };
    let said = flow::run(&plain(), &mut world);
    assert_eq!(
        said,
        Said::Failed("move-old".to_string(), "busy".to_string())
    );
    untouched(&setup);
    assert!(!setup.dir.join(".miyu-old-0.9.0").exists());
}

#[test]
fn an_unknown_system_stops_before_the_network() {
    let setup = Setup::new("system", "1.0.0", false);
    let version_of = |_: &Path| Ok("miyu 1.0.0".to_string());
    let mut ask = |_: &Version, _: &Version| Answer::Yes;
    let mut step = |_: Step| {};
    let mut rename = |from: &Path, to: &Path| fs::rename(from, to);
    let mut world = World {
        exe: setup.dir.join("miyu"),
        net: &setup.net,
        keys: setup.keys.clone(),
        target: None,
        exe_suffix: "",
        current: Version::parse("0.9.0").unwrap(),
        version_of: &version_of,
        ask: &mut ask,
        step: &mut step,
        rename: &mut rename,
    };
    assert_eq!(flow::run(&plain(), &mut world), Said::UnknownSystem);
    assert!(setup.net.asked.borrow().is_empty());
}

#[test]
fn without_a_stable_release_the_hint_says_pre_and_with_nothing_it_says_none() {
    let mut setup = Setup::new("onlypre", "1.0.0-rc.1", true);
    let list = setup.body("per_page=30");
    let only: Vec<serde_json::Value> = serde_json::from_slice::<Vec<serde_json::Value>>(&list)
        .unwrap()
        .into_iter()
        .filter(|release| release["tag_name"] != "v0.8.0")
        .collect();
    setup.answer("per_page=30", Ok(serde_json::to_vec(&only).unwrap()));
    let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0-rc.1", Answer::Yes);
    assert_eq!(said, Said::OnlyPre);
    assert_eq!(said.code(), 1);
    setup.answer("per_page=30", Ok(b"[]".to_vec()));
    let (said, _, _) = run(&setup, plain(), "0.9.0", "miyu 1.0.0-rc.1", Answer::Yes);
    assert_eq!(said, Said::NoRelease);
    untouched(&setup);
}
