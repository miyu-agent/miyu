//! 藏起来的那一格（施工 5-13，PV-RM 起只藏数据根）：钥匙那些别的程序的目录不藏；数据根落在能写的里面的，只在藏得住的地方藏。

use miyu_kernel::event::Level;

use super::*;

/// 只读的规格里藏的只有数据根（施工 PV-RM）：家目录里有 `.ssh` 也不藏。
#[test]
fn only_the_data_root_is_hidden_not_the_key_folders() {
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
    assert_eq!(sandboxed.spec.hidden, [real(".miyu")]);
    std::fs::remove_dir_all(&scratch).expect("删得掉");
}

/// 能写的里面的数据根：macOS 上 Seatbelt 藏得住，照藏（施工 5-14 改）；Windows 照藏；Landlock 照 dsh 不藏（施工 5-14 改）。能写的
/// 以外照藏。
#[test]
fn a_data_root_inside_a_writable_place_is_hidden_only_where_it_can_be() {
    let scratch = std::env::temp_dir().join(format!("miyu-sandbox-mounts-{}", std::process::id()));
    let home = scratch.join("home");
    std::fs::create_dir_all(home.join(".miyu")).expect("建得了目录");
    let real = |dir: &str| std::fs::canonicalize(home.join(dir)).expect("在");
    let write = [std::fs::canonicalize(&home).expect("在")];
    let without: &[PathBuf] = if cfg!(any(target_os = "macos", windows)) {
        &[real(".miyu")]
    } else {
        &[]
    };
    assert_eq!(
        hidden(real(".miyu"), &write),
        without,
        "落在能写的里面的 macOS、Windows 上藏，Landlock 不挖洞（施工 5-14 改）"
    );
    let elsewhere = [std::fs::canonicalize(&scratch).expect("在").join("work")];
    assert_eq!(
        hidden(real(".miyu"), &elsewhere),
        [real(".miyu")],
        "能写的以外：照藏"
    );
    std::fs::remove_dir_all(&scratch).expect("删得掉");
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
