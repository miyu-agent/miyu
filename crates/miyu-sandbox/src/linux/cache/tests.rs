//! 缓存认不认得出过期（施工 V-47）：每一步改完，缓存交出来的那一份建的时候加的规则（路径、放什么、每条落在哪个文件上），要和
//! 此刻从头排、从头打开查的逐条一样。手写的几种情形在这里，随机造目录树和改动的对照在 `same.rs`。
//!
//! 树不摆在临时目录下：一级级走要列它的每一级上级，`/tmp` 里别的程序（同一个测试程序里别的测试）一直在建删东西，缓存次次
//! 重建、建到一半还会对不上。摆在 cargo 的输出目录下（`target/<profile>/miyu-sandbox-cache/`），这里的几个测试排着跑。路上
//! 别处还是可能有人在改（比如别的工作树）：对不上的再取一次，5 次里有一次对上就算（缓存真过期了的话，再取几次交出来的还是
//! 同一份过期的）。

use std::ffi::OsString;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use super::*;
use crate::linux::reads::plan;
use crate::linux::rules::build as add_all;

mod same;

/// 一个用完就删的临时目录（真实的位置）。
pub(super) struct Tree(pub(super) PathBuf);

impl Tree {
    pub(super) fn new(tag: &str) -> Tree {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let exe = std::env::current_exe().expect("知道测试程序在哪");
        let base = exe
            .parent()
            .and_then(Path::parent)
            .expect("测试程序在 target/<profile>/deps/ 里")
            .join("miyu-sandbox-cache");
        let dir = base.join(format!("{tag}-{}-{n}", std::process::id()));
        drop(std::fs::remove_dir_all(&dir));
        std::fs::create_dir_all(&dir).expect("建得了");
        Tree(std::fs::canonicalize(dir).expect("在"))
    }

    pub(super) fn at(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        // 列不出来的目录先改回来，删得掉。
        drop(
            Command::new("chmod")
                .arg("-R")
                .arg("u+rwx")
                .arg(&self.0)
                .stderr(std::process::Stdio::null())
                .status(),
        );
        drop(std::fs::remove_dir_all(&self.0));
    }
}

/// 这台机器的 Landlock 能不能用：不能的不测（CI 上必须能，`tests/linux.rs` 守着）。能的交回排队的锁，测试拿着它跑。
pub(super) fn usable() -> Option<MutexGuard<'static, ()>> {
    static SERIAL: Mutex<()> = Mutex::new(());
    rules::available()
        .is_ok()
        .then(|| SERIAL.lock().unwrap_or_else(PoisonError::into_inner))
}

/// 此刻从头排的缓存那一层的规则，和每条此刻打开查到的（规则集照样建，只是不收紧）。从头建会报错的交回出错。
pub(super) fn fresh(spec: &Spec) -> Result<Vec<(Rule, Outcome)>, String> {
    let rules = rules::base(spec, plan(&spec.hidden)?, Layer::Base);
    let (_, outcomes) = add_all(rules::ruleset(Layer::Base)?, &rules, None)?;
    Ok(rules.into_iter().zip(outcomes).collect())
}

/// 取一次，和此刻从头建的比：交回这次是不是新建的。从头建报错的，缓存要交回没有。
pub(super) fn check(cache: &Cache, spec: &Spec, what: &str) -> bool {
    let mut last = String::new();
    for _ in 0..5 {
        let served = cache.ruleset(spec);
        let now = fresh(spec);
        match (served, now) {
            (None, Err(_)) => return true,
            (Some(served), Ok(now)) if *served.rules == now => return served.built,
            (served, now) => {
                last = format!(
                    "缓存的：{:?}\n从头建：{now:?}",
                    served.map(|served| (served.built, served.rules))
                );
            }
        }
    }
    panic!("{what}：缓存交出来的和此刻从头建的对不上\n{last}");
}

/// 摆一棵：`data`（藏，里面 `token`），旁边 `notes/a.txt`、`b.txt`、`deep/x/y`，能写的 `work`。
fn world() -> (Tree, Spec) {
    let tree = Tree::new("world");
    for dir in ["data", "notes", "deep/x/y", "work", "gate"] {
        std::fs::create_dir_all(tree.at(dir)).expect("建得了");
    }
    std::fs::write(tree.at("data/token"), "secret").expect("写得进");
    std::fs::write(tree.at("notes/a.txt"), "a").expect("写得进");
    std::fs::write(tree.at("b.txt"), "b").expect("写得进");
    let spec = Spec {
        write: vec![tree.at("work"), tree.at("gate/w")],
        hidden: vec![tree.at("deep/x/y/z"), tree.at("data")],
        read: None,
        reveal: Vec::new(),
        sockets: Vec::new(),
    };
    (tree, spec)
}

#[test]
fn an_unchanged_tree_is_served_from_the_cache() {
    let Some(_serial) = usable() else {
        return;
    };
    let (_tree, spec) = world();
    let cache = Cache::new();
    assert!(check(&cache, &spec, "头一次"), "头一次是新建的");
    // 没改：取几次总有不是新建的（`/tmp` 别人在改的那几次除外）。
    assert!(
        (0..10).any(|_| !check(&cache, &spec, "没改")),
        "没改的从缓存交"
    );
    assert_eq!(cache.len(), 1);
}

/// 改一步：说的是什么、怎么改。
type Step<'a> = (&'a str, Box<dyn Fn() + 'a>);

#[test]
fn every_kind_of_change_on_the_way_is_seen() {
    let Some(_serial) = usable() else {
        return;
    };
    let (tree, spec) = world();
    let cache = Cache::new();
    check(&cache, &spec, "头一次");
    let steps: Vec<Step<'_>> = vec![
        (
            "上级里新建目录",
            Box::new(|| std::fs::create_dir(tree.at("new")).expect("建得了")),
        ),
        (
            "上级里新建文件",
            Box::new(|| std::fs::write(tree.at("f"), "f").expect("写得进")),
        ),
        (
            "更深一级新建",
            Box::new(|| std::fs::write(tree.at("deep/x/n"), "n").expect("写得进")),
        ),
        (
            "删掉",
            Box::new(|| std::fs::remove_file(tree.at("b.txt")).expect("删得掉")),
        ),
        (
            "改名",
            Box::new(|| std::fs::rename(tree.at("f"), tree.at("g")).expect("改得了")),
        ),
        (
            "同名换成新目录",
            Box::new(|| {
                std::fs::remove_dir_all(tree.at("notes")).expect("删得掉");
                std::fs::create_dir(tree.at("notes")).expect("建得了");
            }),
        ),
        (
            "同名的文件换成目录",
            Box::new(|| {
                std::fs::remove_file(tree.at("g")).expect("删得掉");
                std::fs::create_dir(tree.at("g")).expect("建得了");
            }),
        ),
        (
            "放读的挪进藏起来的、原处另建同名的",
            Box::new(|| {
                std::fs::rename(tree.at("notes"), tree.at("data/notes")).expect("挪得了");
                std::fs::create_dir(tree.at("notes")).expect("建得了");
            }),
        ),
        (
            "藏起来的改走、原处另建",
            Box::new(|| {
                std::fs::rename(tree.at("data"), tree.at("old")).expect("改得了");
                std::fs::create_dir(tree.at("data")).expect("建得了");
            }),
        ),
        (
            "旁边的整个换成藏起来的",
            Box::new(|| {
                std::fs::rename(tree.at("data"), tree.at("gone")).expect("改得了");
                std::fs::rename(tree.at("notes"), tree.at("data")).expect("改得了");
                std::fs::create_dir(tree.at("notes")).expect("建得了");
            }),
        ),
        (
            "藏起来的换成链接",
            Box::new(|| {
                std::fs::rename(tree.at("data"), tree.at("was")).expect("改得了");
                symlink(tree.at("old"), tree.at("data")).expect("建得了链接");
            }),
        ),
        (
            "旁边的换成指进藏起来的链接",
            Box::new(|| {
                std::fs::remove_dir_all(tree.at("notes")).expect("删得掉");
                symlink(tree.at("old"), tree.at("notes")).expect("建得了链接");
            }),
        ),
        (
            "路上的目录换成链接",
            Box::new(|| {
                std::fs::rename(tree.at("deep"), tree.at("deep2")).expect("改得了");
                symlink(tree.at("deep2"), tree.at("deep")).expect("建得了链接");
            }),
        ),
        (
            "链接改指别处",
            Box::new(|| {
                std::fs::remove_file(tree.at("deep")).expect("删得掉");
                std::fs::create_dir_all(tree.at("elsewhere/x/y")).expect("建得了");
                symlink(tree.at("elsewhere"), tree.at("deep")).expect("建得了链接");
            }),
        ),
        (
            "藏起来的终于建出来了",
            Box::new(|| {
                std::fs::create_dir(tree.at("elsewhere/x/y/z")).expect("建得了");
            }),
        ),
        (
            "能写的删了重建",
            Box::new(|| {
                std::fs::remove_dir_all(tree.at("work")).expect("删得掉");
                std::fs::create_dir(tree.at("work")).expect("建得了");
            }),
        ),
        (
            "能写的换成文件",
            Box::new(|| {
                std::fs::remove_dir_all(tree.at("work")).expect("删得掉");
                std::fs::write(tree.at("work"), "w").expect("写得进");
            }),
        ),
        (
            "同一刻里改两次、再把修改时间拨回去",
            Box::new(|| {
                std::fs::write(tree.at("t1"), "1").expect("写得进");
                std::fs::remove_file(tree.at("t1")).expect("删得掉");
                std::fs::write(tree.at("t2"), "2").expect("写得进");
                let back = Command::new("touch")
                    .args(["-d", "@1000000000"])
                    .arg(&tree.0)
                    .status()
                    .expect("起得来");
                assert!(back.success());
            }),
        ),
        (
            "不在的能写的那一条，上级进不去了（不在 → 查不了）",
            Box::new(|| {
                let locked = Command::new("chmod")
                    .arg("000")
                    .arg(tree.at("gate"))
                    .status();
                assert!(locked.expect("起得来").success());
            }),
        ),
        (
            "又进得去了",
            Box::new(|| {
                let opened = Command::new("chmod")
                    .arg("755")
                    .arg(tree.at("gate"))
                    .status();
                assert!(opened.expect("起得来").success());
            }),
        ),
        (
            "路上的目录列不出来",
            Box::new(|| {
                let locked = Command::new("chmod")
                    .arg("000")
                    .arg(tree.at("elsewhere/x"))
                    .status();
                assert!(locked.expect("起得来").success());
            }),
        ),
        (
            "又列得出来了",
            Box::new(|| {
                let opened = Command::new("chmod")
                    .arg("755")
                    .arg(tree.at("elsewhere/x"))
                    .status();
                assert!(opened.expect("起得来").success());
            }),
        ),
    ];
    for (what, step) in steps {
        step();
        check(&cache, &spec, what);
    }
}

#[test]
fn a_different_spec_is_a_different_entry() {
    let Some(_serial) = usable() else {
        return;
    };
    let (tree, spec) = world();
    let cache = Cache::new();
    let variants = [
        Spec {
            hidden: vec![tree.at("data"), tree.at("notes")],
            ..spec.clone()
        },
        Spec {
            write: Vec::new(),
            ..spec.clone()
        },
        Spec {
            reveal: vec![tree.at("data/token")],
            ..spec.clone()
        },
        Spec {
            hidden: spec.hidden.iter().rev().cloned().collect(),
            ..spec.clone()
        },
    ];
    check(&cache, &spec, "原样");
    for variant in &variants {
        assert!(check(&cache, variant, "变了的规格"), "变了的规格新建一份");
    }
    // `sockets` 不进缓存那一层：只差它的用同一份。
    let with_socket = Spec {
        sockets: vec![tree.at("work/call.sock")],
        ..spec.clone()
    };
    assert!(
        (0..10).any(|_| !check(&cache, &with_socket, "只差套接字")),
        "只差套接字的用同一份"
    );
}

#[test]
fn only_cacheable_specs_are_cached_and_the_oldest_goes_first() {
    let Some(_serial) = usable() else {
        return;
    };
    let (tree, spec) = world();
    let cache = Cache::new();
    for other in [
        Spec {
            read: Some(Vec::new()),
            ..spec.clone()
        },
        Spec {
            hidden: Vec::new(),
            ..spec.clone()
        },
        Spec {
            reveal: vec![PathBuf::from("relative")],
            ..spec.clone()
        },
    ] {
        assert!(cache.ruleset(&other).is_none(), "{other:?}");
    }
    assert_eq!(cache.len(), 0);
    let specs: Vec<Spec> = (0..=CAP)
        .map(|n| Spec {
            hidden: vec![tree.at(&format!("h{n}"))],
            ..spec.clone()
        })
        .collect();
    for spec in &specs {
        check(&cache, spec, "一份份放");
    }
    assert_eq!(cache.len(), CAP, "满了丢最久没用的");
    assert!(check(&cache, &specs[0], "最早那份"), "最早那份丢了，重建");
    // 没用过一阵的丢掉。
    assert_eq!(cache.sweep(Instant::now() + IDLE), 0);
}

#[test]
fn the_cached_ruleset_is_close_on_exec_and_not_a_standard_stream() {
    let Some(_serial) = usable() else {
        return;
    };
    let (_tree, spec) = world();
    let cache = Cache::new();
    let served = cache.ruleset(&spec).expect("建得成");
    assert!(served.fd.as_raw_fd() >= 3);
    let flags = rustix::io::fcntl_getfd(&*served.fd).expect("查得了");
    assert!(flags.contains(rustix::io::FdFlags::CLOEXEC));
    assert!(
        Path::new("/proc/self/fd")
            .join(served.fd.as_raw_fd().to_string())
            .exists()
    );
}

/// `command` 拼出来的参数里 `--ruleset` 后面那个号码；没有的交回没有。
fn ruleset_arg(command: &Command) -> Option<String> {
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let at = args.iter().position(|arg| arg == "--ruleset")?;
    args.get(at + 1).cloned()
}

/// 标了不缓存的规格（施工 V-52，脚本工具：交图目录每次一个新的）经核心的那一个缓存（[`crate::command`]）：不进缓存、不认指纹
/// （`checks_files` 为假），拼出来的和助手从头建的（[`crate::argv`]）一字不差；接连 `CAP + 2` 份各不一样的，`shell` 那份照旧
/// 在缓存里、没重建。认不认重建：拿着头一次拼的命令（里面拿着规则集），那份要是被挤掉重建了，新的号码一定和它不一样。
/// 路上别处有人在改（`shell` 那份因此重建）的那一次再来，5 次里有一次对上就算；标不起作用的话次次都被挤掉。
#[test]
fn uncached_specs_stay_out_of_the_core_cache_and_leave_the_others_in_it() {
    let Some(_serial) = usable() else {
        return;
    };
    let (tree, spec) = world();
    let shell = crate::Sandboxed {
        helper: PathBuf::from("/bin/true"),
        spec,
        env: Vec::new(),
        uncached: false,
    };
    let program = Path::new("/bin/true");
    assert!(crate::checks_files(&shell), "不标的照旧缓存、要认指纹");
    let mut last = String::new();
    for attempt in 0..5 {
        let first = crate::command(&shell, program, &[]).expect("拼得成");
        let held = ruleset_arg(&first).expect("shell 那份带规则集");
        let count = CACHE.len();
        for n in 0..CAP + 2 {
            // 摆在能写的 `work` 里：不在藏起来的上级路上，建它不让 `shell` 那份过期（真的交图目录在藏起来的数据根里，也不在路上）。
            let images = tree.at(&format!("work/images-{attempt}-{n}"));
            std::fs::create_dir_all(&images).expect("建得了");
            let mut script = shell.clone();
            script.spec.write.push(images);
            script.uncached = true;
            assert!(!crate::checks_files(&script), "标了的不认指纹");
            let command = crate::command(&script, program, &[]).expect("拼得成");
            let (helper, args) = crate::argv(&script, program, &[]).expect("拼得成");
            assert_eq!(command.get_program(), helper.as_os_str());
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                "和助手从头建的拼法一样"
            );
            let key = Key::of(&script.spec);
            assert!(
                !CACHE.lock().iter().any(|entry| entry.key == key),
                "标了的不进缓存"
            );
            assert_eq!(CACHE.len(), count, "缓存的条数不变");
        }
        let again = crate::command(&shell, program, &[]).expect("拼得成");
        let now = ruleset_arg(&again).expect("shell 那份带规则集");
        drop(first);
        if now == held {
            // 同一份不标的，照旧进缓存：差别只在标。
            let mut plain = shell.clone();
            plain
                .spec
                .write
                .push(tree.at(&format!("work/images-{attempt}-0")));
            assert!(crate::checks_files(&plain));
            let command = crate::command(&plain, program, &[]).expect("拼得成");
            assert!(ruleset_arg(&command).is_some(), "不标的带规则集");
            let key = Key::of(&plain.spec);
            assert!(CACHE.lock().iter().any(|entry| entry.key == key));
            return;
        }
        last = format!("头一次 {held}，之后 {now}");
    }
    panic!("shell 那份被挤掉、重建了：{last}");
}
