use super::*;

/// 拼一份环境。
fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .collect()
}

/// 挑出来的名字，照先后。
fn names(passed: &[(OsString, OsString)]) -> Vec<String> {
    passed
        .iter()
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect()
}

/// 另外设上的那几个，照 [`SET`] 的先后。
const OURS: [&str; 5] = [
    "GIT_TERMINAL_PROMPT",
    "PAGER",
    "GIT_PAGER",
    "NO_COLOR",
    "TERM",
];

/// `names` 后面接上 [`OURS`]。
fn then_ours(names: &[&str]) -> Vec<String> {
    names
        .iter()
        .chain(OURS.iter())
        .map(|name| (*name).to_owned())
        .collect()
}

#[test]
fn everything_passes_but_secrets_and_our_own() {
    let passed = passed_as(
        vars(&[
            ("PATH", "/usr/bin"),
            ("DEEPSEEK_API_KEY", "secret"),
            ("AWS_ACCESS_KEY_ID", "secret"),
            ("aws_secret_access_key", "secret"),
            ("GH_TOKEN", "secret"),
            ("PGPASSWORD", "secret"),
            ("SMB_PASSWD", "secret"),
            ("GOOGLE_APPLICATION_CREDENTIALS", "/home/a/key.json"),
            ("MIYU_HOME", "/home/a/.miyu"),
            ("KUBECONFIG", "/home/a/.kube/config"),
            ("DISPLAY", ":0"),
            ("WAYLAND_DISPLAY", "wayland-1"),
            ("SSH_AUTH_SOCK", "/run/agent"),
            ("LC_ALL", "C.UTF-8"),
        ]),
        false,
        false,
    );
    assert_eq!(
        names(&passed),
        then_ours(&[
            "PATH",
            "KUBECONFIG",
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "SSH_AUTH_SOCK",
            "LC_ALL"
        ])
    );
}

#[test]
fn the_sandbox_gets_no_desktop_or_agent() {
    let passed = passed_as(
        vars(&[
            ("PATH", "/usr/bin"),
            ("DISPLAY", ":0"),
            ("WAYLAND_DISPLAY", "wayland-1"),
            ("XAUTHORITY", "/run/user/1000/xauth"),
            ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus"),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
            ("SSH_AUTH_SOCK", "/run/agent"),
            ("XDG_CONFIG_HOME", "/home/a/.config"),
            ("DEEPSEEK_API_KEY", "secret"),
        ]),
        true,
        false,
    );
    assert_eq!(names(&passed), then_ours(&["PATH", "XDG_CONFIG_HOME"]));
}

#[test]
fn what_we_set_is_always_ours() {
    let passed = passed_as(
        vars(&[
            ("GIT_TERMINAL_PROMPT", "1"),
            ("PAGER", "less"),
            ("GIT_PAGER", "delta"),
            ("NO_COLOR", ""),
            ("TERM", "xterm-kitty"),
        ]),
        false,
        false,
    );
    assert_eq!(
        passed,
        vars(&[
            ("GIT_TERMINAL_PROMPT", "0"),
            ("PAGER", "cat"),
            ("GIT_PAGER", "cat"),
            ("NO_COLOR", "1"),
            ("TERM", "dumb"),
        ])
    );
}

#[test]
fn windows_names_do_not_care_about_case() {
    let given = vars(&[
        ("Path", "C:\\Windows"),
        ("Miyu_Home", "C:\\miyu"),
        ("Display", ":0"),
        ("pager", "more"),
    ]);
    assert_eq!(
        names(&passed_as(given.clone(), true, true)),
        then_ours(&["Path"])
    );
    // 别处分大小写：`Miyu_Home` 不是我们的，`Display` 不是桌面的，`pager` 不是 `PAGER`。
    assert_eq!(
        names(&passed_as(given, true, false)),
        then_ours(&["Path", "Miyu_Home", "Display", "pager"])
    );
}

#[test]
fn a_secret_word_counts_anywhere_in_the_name() {
    assert_eq!(
        names(&passed_as(
            vars(&[("MY_KEYRING", "x"), ("api_token_v2", "y"), ("XLC_ALL", "z")]),
            false,
            false
        )),
        then_ours(&["XLC_ALL"])
    );
}

#[cfg(unix)]
#[test]
fn names_that_are_not_text_still_pass() {
    use std::os::unix::ffi::OsStringExt;

    let odd = OsString::from_vec(vec![b'A', 0xff]);
    let passed = passed_as([(odd.clone(), OsString::from("v"))], false, false);
    assert_eq!(passed[0].0, odd);
    assert_eq!(passed.len(), 1 + OURS.len());
}

/// 场所本机、主人的调用。
fn caller(model: Option<(&str, &str)>, log: Option<&str>) -> Caller {
    Caller {
        venue: miyu_kernel::id::VenueId::parse("local").unwrap(),
        owner: true,
        model: model.map(|(model, effort)| miyu_tool::InUse {
            model: model.to_owned(),
            effort: effort.to_owned(),
        }),
        log: log.map(std::path::PathBuf::from),
    }
}

#[test]
fn the_model_the_effort_and_the_log_are_told_when_known() {
    let told_all = told(Some(&caller(
        Some(("deepseek-v4", "high")),
        Some("/home/a/sessions/s"),
    )));
    assert_eq!(
        told_all,
        vars(&[
            ("MIYU_MODEL", "deepseek-v4"),
            ("MIYU_EFFORT", "high"),
            ("MIYU_SESSION_LOG", "/home/a/sessions/s"),
        ])
    );
    // 关起来的会话没有日志目录；还没解析过模型的没有模型。
    assert_eq!(
        told(Some(&caller(Some(("m", "default")), None))),
        vars(&[("MIYU_MODEL", "m"), ("MIYU_EFFORT", "default")])
    );
    assert!(told(Some(&caller(None, None))).is_empty());
    assert!(told(None).is_empty());
}
