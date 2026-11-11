//! Linux 上核心按规格缓存的规则集（施工 V-47）真跑助手：经 [`miyu_sandbox::command`] 起的（带缓存的规则集）和助手从头建的，
//! 同一个脚本跑出来的退出码、输出逐字一样；藏起来的上级目录里新建、删除、改名，藏起来的被改名、换成链接、被同名的新目录
//! 替换，能写的被删了重建，规格变了，时间戳不动（同一刻里改两次、改完把修改时间拨回去）：缓存认得出，新建的照规格读得到，
//! 藏起来的照旧读不到。规则集只交给起的那一个助手，命令和别的子进程都拿不到。标了不缓存的（施工 V-52，脚本工具）经
//! `command` 起，拼法和跑出来的都和从头建的一样。
//!
//! 内核没有 Landlock 的机器上不缓存（`command` 拼的和从头建的一样），这里只测拼法。

#![cfg(target_os = "linux")]

mod support;

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use miyu_sandbox::{Sandboxed, Spec};
use support::linux::{landlock, run, run_fresh, run_uncached, spec, text};
use support::{Dir, HELPER, serial};

/// 同一个脚本经缓存的、从头建的、标了不缓存经 `command` 的（施工 V-52）各跑一遍，要一样（退出码、标准输出、标准错误），
/// 交回缓存的那一遍。
fn both(spec: &Spec, script: &str) -> Output {
    let cached = run(spec, script);
    let fresh = run_fresh(spec, script);
    assert_eq!(
        (cached.status.code(), &cached.stdout, &cached.stderr),
        (fresh.status.code(), &fresh.stdout, &fresh.stderr),
        "{script}\n缓存的：{cached:?}\n从头建：{fresh:?}"
    );
    let uncached = run_uncached(spec, script);
    assert_eq!(
        (uncached.status.code(), &uncached.stdout, &uncached.stderr),
        (fresh.status.code(), &fresh.stdout, &fresh.stderr),
        "{script}\n标了不缓存的：{uncached:?}\n从头建：{fresh:?}"
    );
    cached
}

/// 读得到（输出 `want`）。
fn reads(spec: &Spec, path: &Path, want: &str) {
    let out = both(spec, &format!("cat '{}'", path.display()));
    assert_eq!(text(&out.stdout), want, "{}：{out:?}", path.display());
}

/// 读不到（被拦）。
fn blocked(spec: &Spec, path: &Path) {
    let out = both(spec, &format!("cat '{}'", path.display()));
    assert_ne!(out.status.code(), Some(0), "{}：{out:?}", path.display());
    assert!(text(&out.stderr).contains("Permission denied"), "{out:?}");
}

/// `command` 拼出来的参数。
fn args_of(spec: &Spec) -> Vec<String> {
    args_marked(spec, false)
}

/// 同 [`args_of`]，`uncached` 是标不标不缓存（施工 V-52）。
fn args_marked(spec: &Spec, uncached: bool) -> Vec<String> {
    let sandboxed = Sandboxed {
        helper: HELPER.into(),
        spec: spec.clone(),
        env: Vec::new(),
        uncached,
    };
    miyu_sandbox::command(&sandboxed, Path::new("/bin/true"), &[])
        .expect("拼得成")
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

/// 一处摆好的：`base` 下藏 `base/data`（里面有 `token`），旁边 `notes/a.txt`、`b.txt`，能写的 `work`。
struct World {
    base: Dir,
    work: Dir,
}

impl World {
    fn new() -> World {
        let base = Dir::new();
        std::fs::create_dir_all(base.path().join("data")).expect("建得了");
        std::fs::write(base.path().join("data/token"), "secret\n").expect("写得进");
        std::fs::create_dir_all(base.path().join("notes")).expect("建得了");
        std::fs::write(base.path().join("notes/a.txt"), "a\n").expect("写得进");
        std::fs::write(base.path().join("b.txt"), "b\n").expect("写得进");
        World {
            base,
            work: Dir::new(),
        }
    }

    fn at(&self, name: &str) -> PathBuf {
        self.base.path().join(name)
    }

    fn spec(&self) -> Spec {
        spec(&[self.work.path()], &[&self.at("data")])
    }
}

#[test]
fn only_landlock_only_specs_with_hidden_paths_get_a_cached_ruleset() {
    let _serial = serial();
    let world = World::new();
    let plain = |spec: &Spec| {
        let mut want = vec!["run".to_string(), "--spec".to_string()];
        want.push(spec.to_json().expect("写得成"));
        want.extend(["--".to_string(), "/bin/true".to_string()]);
        want
    };
    let cached = world.spec();
    let args = args_of(&cached);
    if landlock() {
        assert_eq!(
            args.get(3).map(String::as_str),
            Some("--ruleset"),
            "{args:?}"
        );
        assert!(args[4].parse::<i32>().is_ok_and(|fd| fd >= 3), "{args:?}");
        let mut rest = args.clone();
        rest.drain(3..5);
        assert_eq!(rest, plain(&cached));
    } else {
        assert_eq!(args, plain(&cached));
    }
    // 标了不缓存的（施工 V-52）：能缓存的规格也和从头建的拼法一样。
    assert_eq!(args_marked(&cached, true), plain(&cached));
    // 关起来的会话、没有藏起来的、相对路径的：和从头建的拼法一样。
    let confined = Spec {
        read: Some(vec![world.work.path().to_path_buf()]),
        ..world.spec()
    };
    let open = spec(&[world.work.path()], &[]);
    let relative = spec(&[world.work.path()], &[Path::new("data")]);
    for other in [confined, open, relative] {
        assert_eq!(args_of(&other), plain(&other), "{other:?}");
    }
}

/// 一套摆好的（[`World`] 加一个放开只读的技能文件夹、另一个能写的），和照它拼的规格。
struct Setup {
    world: World,
    other: Dir,
    skill: PathBuf,
    spec: Spec,
}

impl Setup {
    fn new() -> Setup {
        let world = World::new();
        let skill = world.at("data/skills/notes");
        std::fs::create_dir_all(&skill).expect("建得了");
        std::fs::write(skill.join("SKILL.md"), "steps\n").expect("写得进");
        let other = Dir::new();
        let spec = Spec {
            reveal: vec![skill.clone()],
            ..spec(&[world.work.path(), other.path()], &[&world.at("data")])
        };
        Setup {
            world,
            other,
            skill,
            spec,
        }
    }
}

#[test]
fn a_cached_command_behaves_like_a_fresh_one() {
    let _serial = serial();
    if !landlock() {
        return;
    }
    let scripts: [fn(&Setup) -> String; 11] = [
        |s| format!("cat '{}'", s.world.at("data/token").display()),
        |s| format!("ls '{}'", s.world.at("data").display()),
        |s| {
            format!(
                "cat '{}' '{}'",
                s.world.at("b.txt").display(),
                s.world.at("notes/a.txt").display()
            )
        },
        |s| format!("cat '{0}/SKILL.md' && ls '{0}'", s.skill.display()),
        |s| format!("touch '{}/new'", s.skill.display()),
        |s| format!("touch '{}'", s.world.at("c.txt").display()),
        // 跨目录改名、硬链接（「跨目录改名、链接」那一样两层都要放，施工 V-47「两层」）。
        |s| {
            let (work, other) = (s.world.work.path().display(), s.other.path().display());
            format!(
                "echo hi > '{work}/a' && mkdir '{work}/sub' && mv '{work}/a' '{work}/sub/b' && ln '{work}/sub/b' '{work}/c' && mv '{work}/c' '{other}/d' && cat '{other}/d' && ls '{work}/sub'"
            )
        },
        |s| {
            format!(
                "mv '{}' '{}/stolen'",
                s.world.at("b.txt").display(),
                s.world.work.path().display()
            )
        },
        |s| {
            format!(
                "ln '{}' '{}/linked'",
                s.world.at("notes/a.txt").display(),
                s.world.work.path().display()
            )
        },
        |_| "echo x > /dev/null && echo sink".to_string(),
        |_| "grep NoNewPrivs /proc/self/status".to_string(),
    ];
    for script in scripts {
        // 脚本会改东西：缓存的、从头建的各在一套新摆的里跑，路径换成各自的再比。
        let (one, two) = (Setup::new(), Setup::new());
        let cached = run(&one.spec, &script(&one));
        let fresh = run_fresh(&two.spec, &script(&two));
        let same = |out: &Output, setup: &Setup| {
            let swap = |bytes: &[u8]| {
                text(bytes)
                    .replace(&setup.world.base.path().display().to_string(), "<base>")
                    .replace(&setup.world.work.path().display().to_string(), "<work>")
                    .replace(&setup.other.path().display().to_string(), "<other>")
            };
            (out.status.code(), swap(&out.stdout), swap(&out.stderr))
        };
        assert_eq!(same(&cached, &one), same(&fresh, &two), "{}", script(&one));
    }
}

#[test]
fn the_ruleset_reaches_only_the_helper() {
    let _serial = serial();
    let world = World::new();
    let spec = world.spec();
    if !landlock() {
        return;
    }
    // 命令里看不到它：没有一个描述符是规则集。
    assert!(args_of(&spec).contains(&"--ruleset".to_string()));
    let out = run(&spec, "ls -l /proc/self/fd/ && echo listed");
    assert!(!text(&out.stdout).contains("landlock"), "{out:?}");
    assert!(text(&out.stdout).ends_with("listed\n"), "{out:?}");
    // 缓存着的时候，这个进程别处起的子进程也拿不到。
    assert!(args_of(&spec).contains(&"--ruleset".to_string()));
    let out = Command::new("/bin/sh")
        .args(["-c", "ls -l /proc/self/fd/"])
        .output()
        .expect("起得来");
    assert!(!text(&out.stdout).contains("landlock"), "{out:?}");
}

#[test]
fn new_entries_beside_a_hidden_path_are_readable_and_the_hidden_stays_hidden() {
    let _serial = serial();
    let world = World::new();
    let spec = world.spec();
    if !landlock() {
        return;
    }
    let token = world.at("data/token");
    reads(&spec, &world.at("b.txt"), "b\n");
    // 新建目录、文件：照规格本来就该放的，读得到。
    std::fs::create_dir(world.at("fresh")).expect("建得了");
    std::fs::write(world.at("fresh/f.txt"), "f\n").expect("写得进");
    reads(&spec, &world.at("fresh/f.txt"), "f\n");
    // 同一刻里接连两次（再建一个、删一个）。
    std::fs::write(world.at("one.txt"), "1\n").expect("写得进");
    std::fs::remove_file(world.at("b.txt")).expect("删得掉");
    reads(&spec, &world.at("one.txt"), "1\n");
    blocked(&spec, &token);
    // 改名。
    std::fs::rename(world.at("one.txt"), world.at("two.txt")).expect("改得了");
    reads(&spec, &world.at("two.txt"), "1\n");
    // 改完把修改时间拨回去：缓存不看时间戳，照样认出来。
    std::fs::write(world.at("three.txt"), "3\n").expect("写得进");
    let back = Command::new("touch")
        .args(["-d", "@1000000000"])
        .arg(world.base.path())
        .status()
        .expect("起得来");
    assert!(back.success());
    reads(&spec, &world.at("three.txt"), "3\n");
    blocked(&spec, &token);
}

#[test]
fn a_hidden_path_renamed_relinked_or_replaced_stays_hidden_where_the_spec_says() {
    let _serial = serial();
    let world = World::new();
    let spec = world.spec();
    if !landlock() {
        return;
    }
    let data = world.at("data");
    blocked(&spec, &data.join("token"));
    // 藏起来的被改名：改走的是普通目录了（照规格），原处新建的同名目录照旧藏着。
    std::fs::rename(&data, world.at("old")).expect("改得了");
    std::fs::create_dir(&data).expect("建得了");
    std::fs::write(data.join("token"), "new secret\n").expect("写得进");
    reads(&spec, &world.at("old/token"), "secret\n");
    blocked(&spec, &data.join("token"));
    // 旁边放读的目录挪进藏起来的里面、原处另建一个同名的：挪进去的读不到了，新的读得到。
    std::fs::rename(world.at("notes"), data.join("notes")).expect("挪得了");
    std::fs::create_dir(world.at("notes")).expect("建得了");
    std::fs::write(world.at("notes/a.txt"), "new a\n").expect("写得进");
    blocked(&spec, &data.join("notes/a.txt"));
    reads(&spec, &world.at("notes/a.txt"), "new a\n");
    // 旁边放读的目录整个换成藏起来的那个（先挪走藏起来的，再把旁边的挪过去，原处另建同名的）。
    std::fs::rename(&data, world.at("gone")).expect("改得了");
    std::fs::rename(world.at("notes"), &data).expect("改得了");
    std::fs::create_dir(world.at("notes")).expect("建得了");
    blocked(&spec, &data.join("a.txt"));
    // 藏起来的换成链接，指到旁边：链接跳过，经它读的是旁边那个（照它自己的规则）；指到藏起来的里面的照旧读不到。
    std::fs::rename(&data, world.at("was-data")).expect("改得了");
    std::fs::create_dir(world.at("target")).expect("建得了");
    std::fs::write(world.at("target/t.txt"), "t\n").expect("写得进");
    symlink(world.at("target"), &data).expect("建得了链接");
    both(&spec, &format!("cat '{}/t.txt'", data.display()));
    // 旁边的目录换成指进藏起来的链接（藏起来的现在是 `target`，经链接换成真实位置以后藏的是它）。
    std::fs::remove_dir_all(world.at("notes")).expect("删得掉");
    symlink(world.at("target"), world.at("notes")).expect("建得了链接");
    blocked(&spec, &world.at("notes/t.txt"));
}

#[test]
fn a_writable_path_recreated_or_a_changed_spec_gets_new_rules() {
    let _serial = serial();
    let world = World::new();
    let spec = world.spec();
    if !landlock() {
        return;
    }
    let work = world.work.path();
    let out = both(
        &spec,
        &format!("echo w > '{}/a' && echo ok", work.display()),
    );
    assert_eq!(text(&out.stdout), "ok\n");
    // 能写的删了重建：新的照样能写（缓存的规则落在删掉的那个上，得认出来重建）。
    std::fs::remove_dir_all(work).expect("删得掉");
    std::fs::create_dir(work).expect("建得了");
    let out = both(
        &spec,
        &format!("echo w > '{}/b' && echo ok", work.display()),
    );
    assert_eq!(text(&out.stdout), "ok\n", "{out:?}");
    // 规格变了：多藏一条、少一条能写的、多一条放开只读的，各是各的。
    let more = spec_with(&world, &["data", "notes"], &[work]);
    blocked(&more, &world.at("notes/a.txt"));
    reads(&spec, &world.at("notes/a.txt"), "a\n");
    let unwritable = spec_with(&world, &["data"], &[]);
    let out = both(&unwritable, &format!("echo w > '{}/c'", work.display()));
    assert_ne!(out.status.code(), Some(0), "{out:?}");
    let revealed = Spec {
        reveal: vec![world.at("data/token")],
        ..world.spec()
    };
    reads(&revealed, &world.at("data/token"), "secret\n");
    blocked(&spec, &world.at("data/token"));
}

/// `world` 里藏 `hidden` 那几个名字、能写 `write` 的规格。
fn spec_with(world: &World, hidden: &[&str], write: &[&Path]) -> Spec {
    let hidden: Vec<PathBuf> = hidden.iter().map(|name| world.at(name)).collect();
    let hidden: Vec<&Path> = hidden.iter().map(PathBuf::as_path).collect();
    spec(write, &hidden)
}

/// 连 Unix 套接字（每条命令叠的那一小层，施工 V-47「两层」）：能写的地方的、规格 `sockets` 里的连得上，别处的连不上（内核 7.1
/// 起；更老的内核两边都连得上），缓存的和从头建的一样。机器上没有 `python3` 的不测。
#[test]
fn unix_sockets_are_confined_the_same_way() {
    use std::os::unix::net::UnixListener;
    let _serial = serial();
    let world = World::new();
    let listed = Dir::new();
    let elsewhere = Dir::new();
    if !landlock() {
        return;
    }
    let in_work = world.work.path().join("w.sock");
    let in_spec = listed.path().join("s.sock");
    let outside = elsewhere.path().join("o.sock");
    let _listeners: Vec<UnixListener> = [&in_work, &in_spec, &outside]
        .iter()
        .map(|path| UnixListener::bind(path).expect("绑得上"))
        .collect();
    let spec = Spec {
        sockets: vec![in_spec.clone()],
        ..world.spec()
    };
    let mut seen = Vec::new();
    for path in [&in_work, &in_spec, &outside] {
        let script = format!(
            "command -v python3 >/dev/null || exit 77; python3 -c 'import socket, sys; s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1])' '{}' 2>/dev/null",
            path.display()
        );
        let out = both(&spec, &script);
        if out.status.code() == Some(77) {
            return;
        }
        seen.push(out.status.success());
    }
    assert!(seen[0] && seen[1], "能写的地方的、规格里的连得上：{seen:?}");
    let confined = support::linux::mechanisms()
        .iter()
        .any(|name| name == "landlock-sockets");
    assert_eq!(seen[2], !confined, "别处的看内核：{seen:?}");
}
