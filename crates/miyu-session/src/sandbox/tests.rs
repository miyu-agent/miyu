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
        false,
    );
    let read_only = Permission {
        level: Level::Full,
        read_only: true,
    };
    let sandboxed = sandbox
        .for_call(&read_only, &scratch.to_string_lossy(), &[])
        .expect("写得成")
        .expect("进沙盒");
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    assert_eq!(sandboxed.spec.hidden, [real(".miyu"), real(".ssh")]);
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 能写的里面的私密目录：照人给的算（施工 5-13）；macOS 上 Seatbelt 藏得住，照藏（施工 5-14 改）。能写的里面的数据根：Landlock
/// 照 dsh 不藏（施工 5-14 改）。施工 5-16 有挂载时照藏的那一路随 bubblewrap 去掉了（施工 LV-5）。
#[test]
fn a_private_dir_inside_a_writable_place_is_hidden_only_where_it_can_be() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-mounts-{}", std::process::id()));
    let home = scratch.join("home");
    for dir in [".miyu", ".ssh"] {
        std::fs::create_dir_all(home.join(dir)).expect("建得了目录");
    }
    let sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(home.clone()),
        home.join(".miyu"),
        false,
    );
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    let write = [std::fs::canonicalize(&home).expect("在")];
    let without: &[PathBuf] = if cfg!(target_os = "macos") {
        &[real(".miyu"), real(".ssh")]
    } else if cfg!(windows) {
        &[real(".miyu")]
    } else {
        &[]
    };
    assert_eq!(
        sandbox.hidden(real(".miyu"), &write),
        without,
        "落在能写的里面的只有 macOS 上藏；数据根 Windows 上照藏，Landlock 不挖洞（施工 5-14 改）"
    );
    let elsewhere = [std::fs::canonicalize(&scratch).expect("在").join("work")];
    assert_eq!(
        sandbox.hidden(real(".miyu"), &elsewhere),
        [real(".miyu"), real(".ssh")],
        "能写的以外：照藏"
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
        false,
    );
    sandbox.set_private(vec![home.join("diary")]);
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    assert_eq!(
        sandbox.hidden(real(".miyu"), &[]),
        [real(".miyu"), real("diary")],
        "数据根照藏，.ssh 去掉了不藏，人加的藏"
    );
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 藏起来的不重叠（施工 5-15 下修）：私密目录里加了包住数据根的上级、包住别的私密目录的上级，只留外面那一层：
/// 藏了外层也就藏了里面。
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
        false,
    );
    sandbox.set_private(
        [".config/gh", ".ssh", ".cache", ".config", ".ssh"]
            .map(|dir| home.join(dir))
            .to_vec(),
    );
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    assert_eq!(
        sandbox.hidden(real(".cache/miyu"), &[]),
        [real(".ssh"), real(".cache"), real(".config")],
        "数据根、.config/gh 让给外层，.ssh 只写一次"
    );
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 真的经助手跑（施工 5-15 下修，clicall 报的；施工 LV-5 起只走 Landlock）：数据根的上级加成私密目录，命令照样起得来，上级
/// 里面的读不到。
#[cfg(target_os = "linux")]
#[test]
fn a_command_runs_when_a_private_dir_holds_the_data_root() {
    let helper = miyu_sandbox::testkit::built_helper();
    let probe = miyu_sandbox::probe(&helper, std::time::Duration::from_secs(60)).expect("探得了");
    if probe.mechanisms.is_empty() {
        return;
    }
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-holds-{}", std::process::id()));
    let home = scratch.join("home");
    let work = scratch.join("work");
    for dir in [home.join(".cache/miyu"), work.clone()] {
        std::fs::create_dir_all(dir).expect("建得了目录");
    }
    std::fs::write(home.join(".cache/miyu/token"), "secret").expect("写得进");
    let mut sandbox = Sandbox::new(helper, Some(home.clone()), home.join(".cache/miyu"), false);
    sandbox.set_private(vec![home.join(".cache")]);
    let read_only = Permission {
        level: Level::Full,
        read_only: true,
    };
    let sandboxed = sandbox
        .for_call(&read_only, &work.to_string_lossy(), &[])
        .expect("写得成")
        .expect("进沙盒");
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

/// 数据根的 `run/` 被删了的，下一条命令起来以前补上（施工 R3-1）：配置、密钥的临时文件建在它里面，Windows 的沙盒只禁命令起来
/// 那一刻已有的，命令跑着时才建的 `run/` 里面读得到（2026-11-02 虚拟机上量过）。
#[test]
fn the_run_directory_is_back_before_a_command_starts() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-run-{}", std::process::id()));
    let data = scratch.join(".miyu");
    std::fs::create_dir_all(&data).expect("建得了目录");
    std::fs::write(
        data.join(miyu_store::root::MARKER),
        miyu_store::root::MARKER_TEXT,
    )
    .expect("写得进标记");
    let sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(scratch.clone()),
        data.clone(),
        false,
    );
    let read_only = Permission {
        level: Level::Full,
        read_only: true,
    };
    sandbox
        .for_call(&read_only, &scratch.to_string_lossy(), &[])
        .expect("写得成")
        .expect("进沙盒");
    let made = data.join("run").is_dir();
    std::fs::remove_dir_all(&scratch).expect("删得掉");
    assert!(made, "run/ 在命令起来以前就在");
}

/// 这个会话的 `jobs/` 也在命令起来以前建好（施工 R3-1 补）：后台命令的输出写在里面，命令跑着时才建的 `jobs/` 在 Windows 的
/// 沙盒里读得到（会话目录挖开时，它是路上那一层）。
#[test]
fn the_jobs_directory_is_made_before_a_command_starts() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-jobs-{}", std::process::id()));
    let data = scratch.join(".miyu");
    std::fs::create_dir_all(&data).expect("建得了目录");
    std::fs::write(
        data.join(miyu_store::root::MARKER),
        miyu_store::root::MARKER_TEXT,
    )
    .expect("写得进标记");
    let jobs = data.join("sessions").join("s1").join("jobs");
    let sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(scratch.clone()),
        data.clone(),
        false,
    )
    .with_jobs(jobs.clone());
    let read_only = Permission {
        level: Level::Full,
        read_only: true,
    };
    sandbox
        .for_call(&read_only, &scratch.to_string_lossy(), &[])
        .expect("写得成")
        .expect("进沙盒");
    let made = jobs.is_dir();
    std::fs::remove_dir_all(&scratch).expect("删得掉");
    assert!(made, "jobs/ 在命令起来以前就在");
}
