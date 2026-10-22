//! 藏起来的那一格（施工 5-13）：数据根就是旧版的 `~/.miyu` 这种和私密目录重了的，只写一次。

use miyu_kernel::event::Level;

use super::*;

#[test]
fn a_data_root_that_is_also_a_private_dir_is_hidden_once() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-hidden-{}", std::process::id()));
    let home = scratch.join("home");
    for dir in [".miyu", ".ssh"] {
        std::fs::create_dir_all(home.join(dir)).expect("建得了目录");
    }
    let sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".miyu"),
        (None, false),
    );
    let read_only = Permission {
        level: Level::Workspace,
        read_only: true,
    };
    let sandboxed = sandbox
        .for_call(&read_only, &scratch.to_string_lossy(), &[], &[])
        .expect("写得成")
        .expect("进沙盒");
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    assert_eq!(sandboxed.spec.hidden, [real(".miyu"), real(".ssh")]);
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 能写的里面的私密目录（施工 5-16）：能用挂载时照藏，不能时照人给的算（施工 5-13）。
#[test]
fn a_private_dir_inside_a_writable_place_is_hidden_only_with_mounts() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-mounts-{}", std::process::id()));
    let home = scratch.join("home");
    for dir in [".miyu", ".ssh"] {
        std::fs::create_dir_all(home.join(dir)).expect("建得了目录");
    }
    let sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".miyu"),
        (None, false),
    );
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    let write = [std::fs::canonicalize(&home).expect("在")];
    assert_eq!(
        sandbox.hidden(real(".miyu"), &write, false),
        [real(".miyu")],
        "没有挂载：落在能写的里面的私密目录不藏"
    );
    assert_eq!(
        sandbox.hidden(real(".miyu"), &write, true),
        [real(".miyu"), real(".ssh")],
        "有挂载：照藏"
    );
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 这一轮的私密目录（施工 5-15 下）：换上了的照它藏，出厂的去掉了的不藏、人加的藏。
#[test]
fn the_turn_private_dirs_are_what_gets_hidden() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-turn-{}", std::process::id()));
    let home = scratch.join("home");
    for dir in [".miyu", ".ssh", "diary"] {
        std::fs::create_dir_all(home.join(dir)).expect("建得了目录");
    }
    let mut sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".miyu"),
        (None, false),
    );
    sandbox.set_private(vec![home.join("diary")]);
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    assert_eq!(
        sandbox.hidden(real(".miyu"), &[], false),
        [real(".miyu"), real("diary")],
        "数据根照藏，.ssh 去掉了不藏，人加的藏"
    );
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 沙盒强度（施工 5-16 下）：只用 Landlock 的那一档有挂载也不用；没设的照这台机器能不能用。
#[test]
fn weak_strength_turns_mounts_off() {
    let mut sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        None,
        PathBuf::from("data"),
        (None, false),
    );
    assert!(sandbox.mount(true));
    assert!(!sandbox.mount(false));
    sandbox.set_weak(true);
    assert!(!sandbox.mount(true), "弱沙盒不用挂载");
}

/// 写规格时照这一轮的强度（施工 5-16 下）：机器能用挂载，弱沙盒的规格也不写 `mount`。这个进程里的开关临时设上，测完放回去：
/// 这个包的别的测试不看规格的 `mount`。
#[test]
fn a_weak_turn_writes_no_mount_into_the_spec() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-weak-{}", std::process::id()));
    let home = scratch.join("home");
    std::fs::create_dir_all(home.join(".miyu")).expect("建得了目录");
    let mut sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".miyu"),
        (None, false),
    );
    let workspace = Permission {
        level: Level::Workspace,
        read_only: false,
    };
    let cwd = scratch.to_string_lossy().into_owned();
    miyu_sandbox::set_mounts(true);
    let spec = |sandbox: &Sandbox| {
        sandbox
            .for_call(&workspace, &cwd, &[], &[])
            .expect("写得成")
            .expect("进沙盒")
            .spec
            .mount
    };
    let strong = spec(&sandbox);
    sandbox.set_weak(true);
    let weak = spec(&sandbox);
    miyu_sandbox::set_mounts(false);
    std::fs::remove_dir_all(&scratch).expect("删得掉");
    assert!(strong, "能用挂载、没设弱的写 mount");
    assert!(!weak, "弱沙盒不写");
}

/// 藏起来的不重叠（施工 5-15 下修）：私密目录里加了包住数据根的上级、包住别的私密目录的上级，只留外面那一层。bwrap 先盖外层，
/// 里面那一条的挂载点就不在了，所有命令起不来；别的平台盖了外层也就藏了里面。有挂载、没有挂载都一样。
#[test]
fn a_hidden_dir_inside_another_is_left_to_the_outer_one() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-nested-{}", std::process::id()));
    let home = scratch.join("home");
    for dir in [".cache/miyu", ".config/gh", ".ssh"] {
        std::fs::create_dir_all(home.join(dir)).expect("建得了目录");
    }
    let mut sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".cache/miyu"),
        (None, false),
    );
    sandbox.set_private(
        [".config/gh", ".ssh", ".cache", ".config", ".ssh"]
            .map(|dir| home.join(dir))
            .to_vec(),
    );
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    for mount in [false, true] {
        assert_eq!(
            sandbox.hidden(real(".cache/miyu"), &[], mount),
            [real(".ssh"), real(".cache"), real(".config")],
            "挂载 {mount}：数据根、.config/gh 让给外层，.ssh 只写一次"
        );
    }
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 真的经助手跑（施工 5-15 下修，clicall 报的）：数据根的上级加成私密目录，有能用的 bubblewrap 的机器上命令照样起得来，上级
/// 里面的读不到。规格的 `mount` 在这里设上，不动这个进程里的开关：这个包的别的测试会把它放回去。
#[cfg(target_os = "linux")]
#[test]
fn a_command_runs_when_a_private_dir_holds_the_data_root() {
    let helper = miyu_sandbox::testkit::built_helper();
    let probe = miyu_sandbox::probe(&helper, std::time::Duration::from_secs(60)).expect("探得了");
    if !probe
        .mechanisms
        .iter()
        .any(|mechanism| mechanism == "bwrap")
    {
        return;
    }
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-holds-{}", std::process::id()));
    let home = scratch.join("home");
    let work = scratch.join("work");
    for dir in [home.join(".cache/miyu"), work.clone()] {
        std::fs::create_dir_all(dir).expect("建得了目录");
    }
    std::fs::write(home.join(".cache/miyu/token"), "secret").expect("写得进");
    let mut sandbox = Sandbox::new(
        helper,
        Some(home.clone()),
        home.join(".cache/miyu"),
        (None, false),
    );
    sandbox.set_private(vec![home.join(".cache")]);
    let workspace = Permission {
        level: Level::Workspace,
        read_only: false,
    };
    let mut sandboxed = sandbox
        .for_call(&workspace, &work.to_string_lossy(), &[], &[])
        .expect("写得成")
        .expect("进沙盒");
    sandboxed.spec.mount = true;
    let token = std::fs::canonicalize(home.join(".cache/miyu/token")).expect("在");
    let script = format!(
        "echo ran; cat '{}' 2>/dev/null || echo hidden",
        token.display()
    );
    let (program, args) = miyu_sandbox::argv(
        &sandboxed,
        Path::new("/bin/sh"),
        &["-c".into(), script.into()],
    )
    .expect("写得成 JSON");
    let out = std::process::Command::new(program)
        .args(args)
        .current_dir(&work)
        .envs(sandboxed.env.iter().map(|(name, value)| (name, value)))
        .output()
        .expect("起得来助手");
    std::fs::remove_dir_all(&scratch).expect("删得掉");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "ran\nhidden\n",
        "{out:?}"
    );
}
