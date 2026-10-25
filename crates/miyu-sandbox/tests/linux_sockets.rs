//! Linux 上真跑助手：Unix 套接字（从 `linux.rs` 拆出来守 500 行上限，施工 MB-1）。能替命令在沙盒外读写的系统服务照样挡：规格外
//! 路径上的、沙盒外建的抽象套接字连不上（看内核）；脚本调工具的通道照样连得上；关起来的会话（规格写了 `read`）在哪种内核上都
//! 连不上，Landlock 管不住的照 seccomp。

#![cfg(target_os = "linux")]

mod support;

use miyu_sandbox::Spec;
use support::linux::{bwrap, mechanisms, refused_without_landlock, run, run_with, spec, text};
use support::{Dir, serial};

/// 这台机器的内核版本：`7.2.3-zen1` 这样的写法取前两段。
fn kernel() -> (u32, u32) {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").expect("读得出");
    let mut parts = release
        .trim()
        .split(['.', '-'])
        .map(|part| part.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

/// 在沙盒里用 `python3` 连一个 Unix 套接字，`address` 以 `@` 开头的是抽象的。交回连得上连不上；机器上没有 `python3`
/// 的交回空的。
fn connects(spec: &Spec, address: &str) -> Option<bool> {
    connects_with(spec, address, &[])
}

/// 同 [`connects`]，助手多带几个环境变量 `env`。
fn connects_with(spec: &Spec, address: &str, env: &[(&str, &str)]) -> Option<bool> {
    let script = format!(
        "command -v python3 >/dev/null || exit 77; python3 -c 'import socket, sys; a = sys.argv[1]; s = socket.socket(socket.AF_UNIX); s.connect(\"\\0\" + a[1:] if a.startswith(\"@\") else a)' '{address}'"
    );
    let out = run_with(spec, &script, env);
    match out.status.code() {
        Some(77) => None,
        Some(0) => Some(true),
        _ => {
            let stderr = text(&out.stderr);
            assert!(
                stderr.contains("PermissionError")
                    || stderr.contains("Operation not permitted")
                    || stderr.contains("Permission denied"),
                "连不上的原因不是被拦：{stderr}"
            );
            Some(false)
        }
    }
}

/// 能替命令在沙盒外读写的系统服务照样挡（2026-09-29 项目主人定）：规格外路径上的 Unix 套接字连不上（内核 7.1 起），
/// 沙盒外建的抽象套接字连不上（6.12 起）；更老的内核拦不住，照连得上测。能写的地方的套接字照样连得上。
#[test]
fn system_service_sockets_outside_the_spec_cannot_be_connected() {
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixListener};
    let _serial = serial();
    let work = Dir::new();
    let other = Dir::new();
    let spec = spec(&[work.path()], &[]);
    if refused_without_landlock(&spec) {
        return;
    }
    let outside = other.path().join("bus");
    let _outside = UnixListener::bind(&outside).expect("听得了");
    let inside = work.path().join("mine.sock");
    let _inside = UnixListener::bind(&inside).expect("听得了");
    let name = format!("miyu-sandbox-test-{}", std::process::id());
    let _abstract = UnixListener::bind_addr(
        &SocketAddr::from_abstract_name(name.as_bytes()).expect("名字合法"),
    )
    .expect("听得了");
    let version = kernel();
    let Some(outside_ok) = connects(&spec, &outside.to_string_lossy()) else {
        return;
    };
    assert_eq!(
        outside_ok,
        version < (7, 1),
        "规格外路径上的套接字，内核 {version:?}"
    );
    assert_eq!(
        connects(&spec, &inside.to_string_lossy()),
        Some(true),
        "能写的地方的照样连得上"
    );
    assert_eq!(
        connects(&spec, &format!("@{name}")),
        Some(version < (6, 12)),
        "沙盒外建的抽象套接字，内核 {version:?}"
    );
    // 用挂载收紧的照样拦（施工 5-16：挂好以后里面那一次照旧是 Landlock）。
    if bwrap() {
        let mounted = Spec {
            mount: true,
            sockets: Vec::new(),
            ..spec.clone()
        };
        assert_eq!(
            connects(&mounted, &outside.to_string_lossy()),
            Some(version < (7, 1)),
            "挂载时规格外路径上的套接字"
        );
        assert_eq!(
            connects(&mounted, &format!("@{name}")),
            Some(version < (6, 12)),
            "挂载时沙盒外建的抽象套接字"
        );
    }
}

/// 脚本调工具的通道（施工 M-3）：规格的 `sockets` 写了的那一个，不在能写的地方、落在藏起来的里面，照样连得上；旁边别的照旧
/// 拦（内核 7.1 起）。用挂载收紧的同样。
#[test]
fn a_socket_named_in_the_spec_can_be_connected() {
    use std::os::unix::net::UnixListener;
    let _serial = serial();
    let work = Dir::new();
    let other = Dir::new();
    let hidden = Dir::new();
    let named = other.path().join("call.sock");
    let _named = UnixListener::bind(&named).expect("听得了");
    let beside = other.path().join("beside.sock");
    let _beside = UnixListener::bind(&beside).expect("听得了");
    let secret = hidden.path().join("call.sock");
    let _secret = UnixListener::bind(&secret).expect("听得了");
    let spec = Spec {
        sockets: vec![named.clone(), secret.clone()],
        ..spec(&[work.path()], &[hidden.path()])
    };
    if refused_without_landlock(&spec) {
        return;
    }
    let Some(named_ok) = connects(&spec, &named.to_string_lossy()) else {
        return;
    };
    assert!(named_ok, "写了的连得上");
    assert_eq!(
        connects(&spec, &beside.to_string_lossy()),
        Some(kernel() < (7, 1)),
        "旁边的照旧拦"
    );
    assert_eq!(
        connects(&spec, &secret.to_string_lossy()),
        Some(true),
        "藏起来的里面写了的也连得上"
    );
    if bwrap() {
        let mounted = Spec {
            mount: true,
            ..spec.clone()
        };
        assert_eq!(connects(&mounted, &named.to_string_lossy()), Some(true));
        assert_eq!(
            connects(&mounted, &secret.to_string_lossy()),
            Some(true),
            "挂回来了"
        );
    }
}

/// 关起来的会话（规格写了 `read`，施工 MB-1）：路径上的、抽象的 Unix 套接字在哪种内核上都连不上。Landlock 管得住的（第 9 版的
/// 连套接字、第 6 版的抽象套接字）照它；管不住的照 seccomp 不准新建 AF_UNIX 套接字，`MIYU_SANDBOX_SECCOMP=force` 在新内核上也
/// 走这一路。`socketpair`（Python 的多进程、node 的管道）照常。两样都没有的机器上不跑。
#[test]
fn a_confined_command_reaches_no_unix_socket_on_any_kernel() {
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixListener};
    let _serial = serial();
    let work = Dir::new();
    let other = Dir::new();
    let mut confined = spec(&[work.path()], &[]);
    confined.read = Some(
        [
            "/usr",
            "/bin",
            "/lib",
            "/lib64",
            "/etc",
            "/dev",
            "/proc/self",
        ]
        .into_iter()
        .map(std::path::PathBuf::from)
        .chain([work.path().to_path_buf()])
        .collect(),
    );
    if refused_without_landlock(&confined) {
        return;
    }
    let found = mechanisms();
    if !found
        .iter()
        .any(|name| name == "landlock-sockets" || name == "seccomp")
    {
        let out = run(&confined, "echo ran");
        assert_eq!(
            out.status.code(),
            Some(i32::from(miyu_sandbox::EXIT_HELPER)),
            "{out:?}"
        );
        assert!(out.stdout.is_empty(), "管不住套接字的不跑");
        return;
    }
    let outside = other.path().join("bus");
    let _outside = UnixListener::bind(&outside).expect("听得了");
    // 能写的地方的：Landlock 放行，只有 seccomp 挡得住，分得出到底走的哪一路。
    let inside = work.path().join("mine.sock");
    let _inside = UnixListener::bind(&inside).expect("听得了");
    let name = format!("miyu-sandbox-confined-{}", std::process::id());
    let _abstract = UnixListener::bind_addr(
        &SocketAddr::from_abstract_name(name.as_bytes()).expect("名字合法"),
    )
    .expect("听得了");
    let forced: &[(&str, &str)] = &[("MIYU_SANDBOX_SECCOMP", "force")];
    for env in [&[][..], forced] {
        let Some(outside_ok) = connects_with(&confined, &outside.to_string_lossy(), env) else {
            return;
        };
        assert!(!outside_ok, "{env:?}：规格外路径上的套接字连不上");
        let by_seccomp = !env.is_empty() || !found.iter().any(|name| name == "landlock-sockets");
        assert_eq!(
            connects_with(&confined, &inside.to_string_lossy(), env),
            Some(!by_seccomp),
            "{env:?}：能写的地方的套接字只有走 seccomp 时连不上"
        );
        assert_eq!(
            connects_with(&confined, &format!("@{name}"), env),
            Some(false),
            "{env:?}：沙盒外建的抽象套接字连不上"
        );
        let out = run_with(
            &confined,
            "python3 -c 'import socket; a, b = socket.socketpair(); a.sendall(b\"hi\"); print(b.recv(2).decode())'",
            env,
        );
        assert_eq!(
            (out.status.code(), text(&out.stdout)),
            (Some(0), "hi\n".to_string()),
            "{env:?}：{out:?}"
        );
    }
}
