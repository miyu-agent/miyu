//! 排函数的流程里不编、不录的几样（施工 LW-6）：终端界面认不认清单、量尺的命令行、清单空着要报。

use super::*;

/// 测试的临时目录，用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("xtask-order-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("清得掉上次留下的");
        }
        fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}

#[test]
fn a_checkout_takes_the_order_only_when_its_build_script_reads_the_variable() {
    let scratch = Scratch::new("takes");
    assert!(!takes_order(&scratch.0).unwrap(), "没有 build.rs");
    fs::write(scratch.0.join("build.rs"), "fn main() {}\n").unwrap();
    assert!(!takes_order(&scratch.0).unwrap(), "build.rs 不认这个变量");
    fs::write(
        scratch.0.join("build.rs"),
        "const VARIABLE: &str = \"MIYU_LINK_ORDER\";\nfn main() {}\n",
    )
    .unwrap();
    assert!(takes_order(&scratch.0).unwrap());
}

#[test]
fn a_build_script_that_cannot_be_read_is_an_error() {
    let scratch = Scratch::new("unreadable");
    // 是个目录：在却读不了，不当成「不认」悄悄不排。
    fs::create_dir_all(scratch.0.join("build.rs")).unwrap();
    let error = takes_order(&scratch.0).unwrap_err();
    assert!(error.starts_with("读不了 "), "{error}");
}

#[test]
fn the_tui_list_is_blank_and_named_after_the_program() {
    let scratch = Scratch::new("list");
    let checkout = scratch.0.join("tui");
    let target = scratch.0.join("target");
    fs::create_dir_all(&checkout).unwrap();
    assert_eq!(tui_list(&checkout, &target).unwrap(), None);
    assert!(!target.exists(), "不认的不写清单");
    fs::write(checkout.join("build.rs"), "MIYU_LINK_ORDER").unwrap();
    let list = tui_list(&checkout, &target).unwrap().unwrap();
    assert_eq!(list, target.join("order").join("miyu-tui.txt"));
    assert_eq!(fs::read_to_string(&list).unwrap(), "");
}

#[test]
fn an_empty_list_is_refused() {
    let scratch = Scratch::new("empty");
    let list = scratch.0.join("order").join("miyu.txt");
    blank(&list).unwrap();
    assert!(listed(&list).unwrap_err().contains("却是空的"));
    fs::write(&list, "\n  \n").unwrap();
    assert!(listed(&list).is_err(), "只有空行也算空");
    fs::write(&list, "main\n").unwrap();
    listed(&list).unwrap();
    assert!(
        listed(&scratch.0.join("nowhere.txt"))
            .unwrap_err()
            .starts_with("读不了 ")
    );
}

#[test]
fn the_recording_command_names_every_path() {
    let paths = Recording {
        miyu: Path::new("/d/miyu"),
        resources: Path::new("/d/resources"),
        work: Path::new("/t/perf-order"),
        tree: Path::new("/r/crates"),
        list: Path::new("/t/order/miyu.txt"),
        flag: "--emulator",
        recorder: Path::new("/usr/bin/qemu-x86_64"),
    };
    let command = recording(Path::new("/t/release/miyu-perf"), &paths);
    assert_eq!(command.get_program(), "/t/release/miyu-perf");
    let args: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_str().unwrap())
        .collect();
    assert_eq!(
        args,
        [
            "--miyu",
            "/d/miyu",
            "--resources",
            "/d/resources",
            "--work",
            "/t/perf-order",
            "--tree",
            "/r/crates",
            "--order",
            "/t/order/miyu.txt",
            "--emulator",
            "/usr/bin/qemu-x86_64",
        ]
    );
}
