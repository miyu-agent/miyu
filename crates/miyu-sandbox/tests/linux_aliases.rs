//! 藏起来的另有挂载别名（施工 SEC-3，外部审计 M-1，`docs/blueprint/sandbox/linux.md`「怎么走」第 1 条）：同一片文件 bind 到了
//! 另一处，Landlock 的规则跟着目录本身走、不认路径，一级级放行把别名那一处整个放了的话，藏起来的从哪条路都读得到。在
//! `unshare -rm` 造的用户、挂载命名空间里 bind 一个别名再跑助手，不动真系统；这台机器不让普通用户建命名空间的（Ubuntu
//! 24.04 的 AppArmor 默认不让）跳过。

#![cfg(target_os = "linux")]

mod support;

use std::process::{Command, Stdio};

use support::linux::{landlock, spec, text};
use support::{Dir, HELPER, serial};

#[test]
fn a_hidden_path_is_not_readable_through_a_bind_mounted_alias() {
    let _serial = serial();
    if !landlock() {
        return;
    }
    let base = Dir::new();
    let home = base.path().join("home");
    let data = home.join(".miyu");
    let alias = base.path().join("alias");
    let work = base.path().join("work");
    for dir in [&data, &alias, &work] {
        std::fs::create_dir_all(dir).expect("建得了");
    }
    std::fs::write(data.join("token"), "secret").expect("写得进");
    std::fs::write(home.join("note.txt"), "note").expect("写得进");
    let spec = spec(&[&work], &[&data]);
    let script = format!(
        "cat '{d}/token'; cat '{a}/.miyu/token'; cat '{a}/note.txt'",
        d = data.display(),
        a = alias.display()
    );
    let out = Command::new("unshare")
        .env("LC_ALL", "C")
        .args(["-rm", "--propagation", "private", "/bin/sh", "-c"])
        .arg(r#"mount --bind "$1" "$2" || exit 99; shift 2; exec "$@""#)
        .arg("sh")
        .arg(&home)
        .arg(&alias)
        .arg(HELPER)
        .args(["run", "--spec", &spec.to_json().expect("写得成"), "--"])
        .args(["/bin/sh", "-c", &script])
        .stdin(Stdio::null())
        .output();
    let Ok(out) = out else {
        return;
    };
    if out.status.code() == Some(99) || text(&out.stderr).starts_with("unshare: ") {
        // 不让建命名空间、挂不上：这台机器测不了。
        return;
    }
    let stdout = text(&out.stdout);
    assert!(
        !stdout.contains("secret"),
        "藏起来的读到了：{stdout:?} {}",
        text(&out.stderr)
    );
    assert_eq!(stdout, "note", "别名里别的照旧读得到：{out:?}");
    assert_eq!(
        text(&out.stderr).matches("Permission denied").count(),
        2,
        "原路径、别名都读不了：{out:?}"
    );
}
