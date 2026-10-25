//! 关起来的会话的 Unix 套接字管不管得住（施工 MB-1）：Linux 上只有 Landlock、没有管套接字的那两样的，管不住。

use super::*;

#[test]
fn linux_needs_landlock_sockets_or_seccomp_to_confine_sockets() {
    let names = |list: &[&str]| list.iter().map(|name| name.to_string()).collect::<Vec<_>>();
    assert!(sockets_confined_by(&names(&[
        "landlock",
        "landlock-sockets"
    ])));
    assert!(sockets_confined_by(&names(&[
        "landlock", "bwrap", "seccomp"
    ])));
    assert_eq!(
        sockets_confined_by(&names(&["landlock", "bwrap"])),
        !cfg!(target_os = "linux"),
        "只有 Landlock 的 Linux 管不住"
    );
}
