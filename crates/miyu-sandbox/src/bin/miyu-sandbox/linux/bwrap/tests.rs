use super::*;

fn spec(write: &[&Path], hidden: &[&Path], reveal: &[&Path]) -> Spec {
    let list = |paths: &[&Path]| paths.iter().map(|path| path.to_path_buf()).collect();
    Spec {
        write: list(write),
        hidden: list(hidden),
        read: None,
        reveal: list(reveal),
        mount: true,
        sockets: Vec::new(),
    }
}

fn texts(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn mounts_go_whole_disk_then_writable_then_covers_then_holes_then_read_only() {
    let base = std::env::temp_dir().join(format!("miyu-bwrap-args-{}", std::process::id()));
    let home = base.join("home");
    let data = home.join(".miyu");
    let ws = data.join("workspace/s1");
    let skill = data.join("packages/kit/skills/x");
    let key = home.join(".ssh-key");
    for dir in [&ws, &skill] {
        fs::create_dir_all(dir).expect("建得了");
    }
    fs::write(&key, "k").expect("写得进");
    let missing = home.join("gone");
    let args = texts(&mounts(&spec(
        &[&home, &ws],
        &[&data, &key, &missing],
        &[&skill],
    )));
    let at = |path: &Path| path.display().to_string();
    let position = |flag: &str, path: &Path| {
        args.windows(2)
            .position(|pair| pair[0] == flag && pair[1] == at(path))
            .unwrap_or_else(|| panic!("没有 {flag} {}：{args:?}", path.display()))
    };
    assert_eq!(
        &args[..7],
        [
            "--ro-bind",
            "/",
            "/",
            "--dev-bind",
            "/dev",
            "/dev",
            "--die-with-parent"
        ]
    );
    let home_bind = position("--bind", &home);
    let cover = position("--tmpfs", &data);
    let ws_bind = position("--bind", &ws);
    let skill_bind = position("--ro-bind", &skill);
    let ro = position("--remount-ro", &data);
    assert!(
        home_bind < cover && cover < ws_bind && ws_bind < skill_bind && skill_bind < ro,
        "{args:?}"
    );
    let file = args
        .windows(3)
        .any(|three| three[0] == "--ro-bind" && three[1] == "/dev/null" && three[2] == at(&key));
    assert!(file, "是文件的拿 /dev/null 盖：{args:?}");
    assert!(!args.contains(&at(&missing)), "不在的跳过");
    #[expect(clippy::let_underscore_must_use, reason = "删不掉就留在临时目录里")]
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn the_inner_spec_keeps_only_what_landlock_still_does() {
    let outer = spec(
        &[Path::new("/w")],
        &[Path::new("/w/h")],
        &[Path::new("/w/h/r")],
    );
    let inner = inner(&outer);
    assert_eq!(inner.write, outer.write);
    assert!(inner.hidden.is_empty() && inner.reveal.is_empty() && !inner.mount);
}

/// 藏起来的另挂在别处的（施工 SEC-3，外部审计 M-1）：别名那一处同样盖 tmpfs、最后设成只读。
#[test]
fn an_alias_of_a_hidden_path_is_covered_too() {
    let base = std::env::temp_dir().join(format!("miyu-bwrap-alias-{}", std::process::id()));
    let data = base.join("home/.miyu");
    let alias = base.join("alias/.miyu");
    for dir in [&data, &alias] {
        fs::create_dir_all(dir).expect("建得了");
    }
    let args = texts(&mounts_with(
        &spec(&[], &[&data], &[]),
        &[data.clone(), alias.clone()],
    ));
    let at = alias.display().to_string();
    let covered = args
        .windows(2)
        .any(|pair| pair[0] == "--tmpfs" && pair[1] == at);
    let frozen = args
        .windows(2)
        .any(|pair| pair[0] == "--remount-ro" && pair[1] == at);
    assert!(covered && frozen, "{args:?}");
    fs::remove_dir_all(&base).expect("删得掉");
}
