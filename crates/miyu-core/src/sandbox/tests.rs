//! 探不成的几种都记一行 `WARN sandbox unavailable`（施工 5-1）。跑整套测试时助手总在主程序旁边，找不到的那一行
//! 在真的核心里走不到，在这里直接探一个旁边没有助手的位置。探成了的那一行见 `crates/miyu/tests/core.rs`。
//! 探到了手段的交回能用和助手，手段是空的、找不到、探不成的交回用不了和原因（施工 5-4 上、下）。沙盒的缓存放在缓存目录
//! 下的 `sandbox`，带上你的 cargo 目录；算不出来的记一行（施工 5-4 下）。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use miyu_log::{LevelFilter, Memory};

use super::*;

/// 探一次就设一遍进程里那一份「关起来的会话的套接字管不管得住」（[`miyu_sandbox::set_confines_sockets`]）：探的测试并行跑
/// 会互相改它（施工 BUG-3），所以一律串着探，在锁里读出来再设回没设过的样子（管得住）。
static PROBING: Mutex<()> = Mutex::new(());

/// 在锁里跑一次探测 `run`，交回它的结果和探完时关起来的会话能不能跑命令（[`miyu_sandbox::confines_others`]）。
fn serially<T>(run: impl FnOnce() -> T) -> (T, bool) {
    let _probing = PROBING.lock().unwrap_or_else(PoisonError::into_inner);
    let found = run();
    let others = miyu_sandbox::confines_others();
    miyu_sandbox::set_confines_sockets(true);
    (found, others)
}

/// 探一次 `exe`，交回探到的结果和记下的行。
fn probed(exe: Option<&Path>) -> (Availability, Vec<String>) {
    let (found, lines, _) = probed_with_others(exe);
    (found, lines)
}

/// 探一次 `exe`，交回探到的结果、记下的行和探完时关起来的会话能不能跑命令。
fn probed_with_others(exe: Option<&Path>) -> (Availability, Vec<String>, bool) {
    let memory = Memory::new();
    let (helper, others) = serially(|| {
        tracing::subscriber::with_default(
            miyu_log::subscriber(memory.clone(), LevelFilter::INFO, None),
            || probe(exe, None),
        )
    });
    (helper, memory.lines(), others)
}

/// 一个用完就删的临时目录，里面有一个假的主程序 `miyu`。
struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-core-sandbox-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建得了目录");
        std::fs::write(dir.join("miyu"), b"").expect("写得进");
        Dir(dir)
    }

    fn exe(&self) -> PathBuf {
        self.0.join("miyu")
    }
}

impl Drop for Dir {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn no_helper_beside_the_program_is_logged_as_not_found() {
    let dir = Dir::new();
    for exe in [None, Some(dir.exe())] {
        let (found, lines) = probed(exe.as_deref());
        assert_eq!(found, Availability::Unusable(Unusable::HelperMissing));
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].contains(" WARN ")
                && lines[0].ends_with(" sandbox unavailable reason=\"helper not found\""),
            "{lines:?}"
        );
    }
}

#[test]
fn a_helper_that_cannot_run_is_logged_with_the_reason() {
    let dir = Dir::new();
    std::fs::write(dir.0.join(miyu_sandbox::HELPER), b"just text\n").expect("写得进");
    let (found, lines) = probed(Some(&dir.exe()));
    assert_eq!(found, Availability::Unusable(Unusable::HelperFailed));
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains(" WARN ")
            && lines[0].contains(" sandbox unavailable reason=\"cannot run helper: "),
        "{lines:?}"
    );
}

/// 在 `dir` 里放一个假的助手：`probe` 时印 `line`。经 `sh` 写：这个进程不拿着它的写端，别的测试这时起进程，也不会让
/// 它执行不了（ETXTBSY）。
#[cfg(unix)]
fn fake_helper(dir: &Dir, line: &str) {
    let status = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            r#"printf '#!/bin/sh\ncat <<"EOF"\n%s\nEOF\n' "$2" > "$1" && chmod 755 "$1""#,
            "sh",
        ])
        .arg(dir.0.join(miyu_sandbox::HELPER))
        .arg(line)
        .status()
        .expect("起得来");
    assert!(status.success());
}

/// 助手说的一行：这一版、这台机器的平台、手段 `mechanisms`，和核心同一个构建 id（施工 5-16 再补）。
#[cfg(unix)]
fn said(mechanisms: &[&str]) -> String {
    serde_json::json!({
        "version": miyu_sandbox::VERSION,
        "platform": miyu_sandbox::Platform::current().name(),
        "mechanisms": mechanisms,
        "build": miyu_sandbox::BUILD,
    })
    .to_string()
}

/// 助手和核心不是同一份源码编的（施工 5-16 再补，网页的 demo 树只编了 `-p miyu`，助手停在旧版）：用不了，记一行说清是哪个
/// 文件、怎么重编；不再每条命令都报 bad spec 退出 125。
#[cfg(unix)]
#[test]
fn a_helper_of_another_build_is_logged_with_how_to_rebuild() {
    let dir = Dir::new();
    let old = serde_json::json!({
        "version": miyu_sandbox::VERSION,
        "platform": miyu_sandbox::Platform::current().name(),
        "mechanisms": ["landlock"],
    });
    fake_helper(&dir, &old.to_string());
    let (found, lines) = probed(Some(&dir.exe()));
    assert_eq!(found, Availability::Unusable(Unusable::HelperFailed));
    assert_eq!(lines.len(), 1, "{lines:?}");
    let helper = dir.0.join(miyu_sandbox::HELPER).display().to_string();
    assert!(
        lines[0].contains(" WARN ")
            && lines[0].contains(&format!(
                " sandbox unavailable reason=\"helper {helper} is from another build"
            ))
            && lines[0].ends_with("rebuild it: cargo build -p miyu-sandbox\""),
        "{lines:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_helper_with_mechanisms_is_handed_on() {
    let dir = Dir::new();
    fake_helper(&dir, &said(&["landlock", "landlock-sockets"]));
    let (found, lines, others) = probed_with_others(Some(&dir.exe()));
    assert_eq!(
        found,
        Availability::Usable(dir.0.join(miyu_sandbox::HELPER)),
        "{lines:?}"
    );
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains(" INFO ") && lines[0].ends_with(" mechanisms=landlock,landlock-sockets"),
        "{lines:?}"
    );
    assert!(others, "管得住套接字");
}

/// 沙盒能用、关起来的会话的 Unix 套接字却管不住的（施工 MB-1：内核的 Landlock 太老、又没有 seccomp）：照样能用（管理员本人的
/// 会话照跑），成员的、场所的会话不跑命令，记一行 `WARN`，好查谁的成员为什么跑不了。
#[cfg(target_os = "linux")]
#[test]
fn a_sandbox_that_cannot_confine_sockets_is_logged_and_keeps_others_off() {
    let dir = Dir::new();
    fake_helper(&dir, &said(&["landlock"]));
    let (found, lines, others) = probed_with_others(Some(&dir.exe()));
    assert_eq!(
        found,
        Availability::Usable(dir.0.join(miyu_sandbox::HELPER))
    );
    assert!(!others, "成员、场所的会话不跑命令");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[1].contains(" WARN ")
            && lines[1].contains(
                " sandbox cannot confine unix sockets: members and venue sessions run no commands"
            )
            && lines[1].ends_with(" mechanisms=landlock"),
        "{lines:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_helper_without_mechanisms_is_not_handed_on() {
    let dir = Dir::new();
    fake_helper(&dir, &said(&[]));
    let (found, lines) = probed(Some(&dir.exe()));
    assert_eq!(
        found,
        Availability::Unusable(Unusable::NoMechanism),
        "照沙盒用不了办：{lines:?}"
    );
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains(" INFO ") && lines[0].contains(" sandbox helper="),
        "{lines:?}"
    );
}

/// 根目录下面的 `parts`：Windows 上得带盘符才算绝对路径。
fn at(parts: &[&str]) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" });
    parts.iter().fold(root, |path, part| path.join(part))
}

/// 一份环境：Linux 的写法，家目录是 `home`，别的都没设。
fn linux(home: Option<PathBuf>) -> Env {
    Env {
        platform: miyu_store::env::Platform::Linux,
        miyu_home: None,
        home,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    }
}

/// 照 `env`、`cargo_home` 算一次沙盒的缓存，交回算出来的和记下的行。
fn cached(
    env: &Env,
    cargo_home: Option<std::ffi::OsString>,
) -> (Option<(PathBuf, Option<PathBuf>)>, Vec<String>) {
    let memory = Memory::new();
    let found = tracing::subscriber::with_default(
        miyu_log::subscriber(memory.clone(), LevelFilter::INFO, None),
        || cache(env, cargo_home),
    );
    (found, memory.lines())
}

#[test]
fn the_sandbox_cache_is_under_the_cache_dir_and_knows_your_cargo() {
    let home = at(&["home", "alice"]);
    let env = linux(Some(home.clone()));
    let root = home.join(".cache").join("miyu").join("sandbox");
    let (found, lines) = cached(&env, None);
    assert_eq!(found, Some((root.clone(), Some(home.join(".cargo")))));
    assert!(lines.is_empty(), "{lines:?}");
    // `CARGO_HOME` 设了的照它；空的当没设。
    let cargo = at(&["opt", "cargo"]);
    assert_eq!(
        cached(&env, Some(cargo.clone().into_os_string())).0,
        Some((root.clone(), Some(cargo)))
    );
    assert_eq!(
        cached(&env, Some(std::ffi::OsString::new())).0,
        Some((root, Some(home.join(".cargo"))))
    );
}

#[test]
fn no_cache_dir_is_logged_and_gives_none() {
    let (found, lines) = cached(&linux(None), Some(at(&["opt", "cargo"]).into_os_string()));
    assert_eq!(found, None);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains(" WARN ")
            && lines[0].ends_with(" sandbox cache unavailable reason=\"no home directory\""),
        "{lines:?}"
    );
}

/// 照上一次探到的用（施工 V-5 上）：探一次 `exe`，状态目录是 `state`，交回探到的结果和记下的行。
#[cfg(unix)]
fn probed_in(exe: &Path, state: &Path) -> (Availability, Vec<String>) {
    let memory = Memory::new();
    let (found, _) = serially(|| {
        tracing::subscriber::with_default(
            miyu_log::subscriber(memory.clone(), LevelFilter::INFO, None),
            || probe(Some(exe), Some(state)),
        )
    });
    (found, memory.lines())
}

/// 一个慢的假助手：`probe` 时先睡 `secs` 秒再印 `line`。
#[cfg(unix)]
fn slow_helper(dir: &Dir, line: &str, secs: &str) {
    let status = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            r#"printf '#!/bin/sh\nsleep %s\ncat <<"EOF"\n%s\nEOF\n' "$3" "$2" > "$1" && chmod 755 "$1""#,
            "sh",
        ])
        .arg(dir.0.join(miyu_sandbox::HELPER))
        .arg(line)
        .arg(secs)
        .status()
        .expect("起得来");
    assert!(status.success());
}

#[cfg(unix)]
#[test]
fn a_remembered_probe_is_used_without_waiting_and_refreshed_behind() {
    let dir = Dir::new();
    let state = dir.0.join("state");
    slow_helper(&dir, &said(&["landlock", "landlock-sockets"]), "1");
    let started = std::time::Instant::now();
    let (found, lines) = probed_in(&dir.exe(), &state);
    assert!(
        started.elapsed() >= std::time::Duration::from_secs(1),
        "头一次照常探"
    );
    assert_eq!(
        found,
        Availability::Usable(dir.0.join(miyu_sandbox::HELPER))
    );
    assert!(!lines.iter().any(|l| l.contains("remembered")), "{lines:?}");
    let file = state.join(remembered::FILE);
    assert!(file.is_file(), "探成了记下");
    std::fs::remove_file(&file).expect("删得掉");
    // 记下一份再起来：不等助手，照记下的用，多一格 remembered；后台再探一遍、重新记下。
    let key = remembered::key(&dir.0.join(miyu_sandbox::HELPER)).expect("认得出");
    remembered::remember(
        &state,
        &key,
        &miyu_sandbox::Probe::new(vec!["landlock".to_string(), "landlock-sockets".to_string()]),
    );
    let started = std::time::Instant::now();
    let (found, lines) = probed_in(&dir.exe(), &state);
    assert!(
        started.elapsed() < std::time::Duration::from_millis(500),
        "不等助手"
    );
    assert_eq!(
        found,
        Availability::Usable(dir.0.join(miyu_sandbox::HELPER))
    );
    assert!(
        lines.len() == 1 && lines[0].contains(" INFO ") && lines[0].contains(" remembered=true"),
        "{lines:?}"
    );
    std::fs::remove_file(&file).expect("删得掉");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !file.is_file() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(file.is_file(), "后台再探一遍，重新记下");
}

#[cfg(unix)]
#[test]
fn a_changed_helper_is_probed_again_and_a_failed_probe_is_not_remembered() {
    let dir = Dir::new();
    let state = dir.0.join("state");
    fake_helper(&dir, &said(&[]));
    assert_eq!(
        probed_in(&dir.exe(), &state).0,
        Availability::Unusable(Unusable::NoMechanism)
    );
    // 换了助手（大小、改动时间都变了）：不照记下的，照常探，探到的是新的。
    fake_helper(&dir, &said(&["landlock", "seccomp"]));
    let (found, lines) = probed_in(&dir.exe(), &state);
    assert_eq!(
        found,
        Availability::Usable(dir.0.join(miyu_sandbox::HELPER))
    );
    assert!(!lines.iter().any(|l| l.contains("remembered")), "{lines:?}");
    // 探不成的不记。
    let other = Dir::new();
    let fresh = other.0.join("state");
    std::fs::write(other.0.join(miyu_sandbox::HELPER), b"just text\n").expect("写得进");
    assert_eq!(
        probed_in(&other.exe(), &fresh).0,
        Availability::Unusable(Unusable::HelperFailed)
    );
    assert!(!fresh.join(remembered::FILE).exists(), "探不成的不记");
}

#[test]
fn a_remembered_probe_is_only_recalled_for_the_same_helper_version_and_build() {
    let dir = Dir::new();
    let state = dir.0.join("state");
    let helper = dir.0.join(miyu_sandbox::HELPER);
    std::fs::write(&helper, b"one").expect("写得进");
    let key = remembered::key(&helper).expect("认得出");
    let probe = miyu_sandbox::Probe::new(vec!["landlock".to_string()]);
    remembered::remember(&state, &key, &probe);
    assert_eq!(remembered::recall(&state, &key), Some(probe));
    std::fs::write(&helper, b"two, longer").expect("写得进");
    let changed = remembered::key(&helper).expect("认得出");
    assert_ne!(changed, key, "助手换了，认不出");
    assert_eq!(remembered::recall(&state, &changed), None);
    // 说法的版本不对、构建 id 不对（施工 5-16 再补：核心换了、助手没换）、读不懂的都当没记过。
    let mut old = miyu_sandbox::Probe::new(Vec::new());
    old.version = miyu_sandbox::VERSION + 1;
    remembered::remember(&state, &key, &old);
    assert_eq!(remembered::recall(&state, &key), None);
    let mut other = miyu_sandbox::Probe::new(vec!["landlock".to_string()]);
    other.build = String::new();
    remembered::remember(&state, &key, &other);
    assert_eq!(remembered::recall(&state, &key), None);
    std::fs::write(state.join(remembered::FILE), b"{not json").expect("写得进");
    assert_eq!(remembered::recall(&state, &key), None);
}
