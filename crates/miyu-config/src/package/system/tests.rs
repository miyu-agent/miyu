//! `[package] systems`（施工 PL-2）：照固定的先后读；不认识的、重复的、和 `targets` 对不上的报 `bad_systems`、说是哪一行；本机
//! 支不支持照 `systems`、`targets` 一起算。

use crate::package::{Code, System, Target, fits_here, read};

/// 一份只带技能的包，`[package]` 里再加 `rest`。
fn bundle(rest: &str) -> String {
    format!("[package]\nname = {{ en = \"Gravity\" }}\n{rest}")
}

#[test]
fn systems_are_read_in_order_and_unwritten_means_none() {
    assert!(
        read(&bundle("")).unwrap().systems.is_empty(),
        "没写的是空的"
    );
    let two = read(&bundle("systems = [\"macos\", \"linux\"]\n")).unwrap();
    assert_eq!(two.systems, [System::Linux, System::Macos], "照固定的先后");
    for system in System::ALL {
        assert_eq!(System::parse(system.as_str()), Some(system));
    }
    assert_eq!(Code::BadSystems.as_str(), "bad_systems");
}

#[test]
fn unknown_repeated_and_mismatched_systems_are_refused() {
    for rest in [
        "systems = [\"freebsd\"]\n",
        "systems = [\"linux\", \"linux\"]\n",
        "targets = [\"windows-x86_64\"]\nsystems = [\"linux\"]\n",
    ] {
        let problem = read(&bundle(rest)).unwrap_err();
        assert_eq!(problem.code, Code::BadSystems, "{rest}");
        let line = rest
            .lines()
            .position(|line| line.starts_with("systems"))
            .unwrap()
            + 3;
        assert_eq!(problem.line, Some(line), "报在 systems 那一行：{rest}");
    }
    let wrong_type = read(&bundle("systems = \"linux\"\n")).unwrap_err();
    assert_eq!(wrong_type.code, Code::NotTexts);
    // 一样的系统、不同处理器的两种包文件照收。
    let fine = read(&bundle(
        "targets = [\"linux-x86_64\", \"linux-aarch64\"]\nsystems = [\"linux\"]\n",
    ))
    .unwrap();
    assert_eq!(fine.systems, [System::Linux]);
}

#[test]
fn support_here_follows_systems_then_targets() {
    let linux = (Some(System::Linux), Some(Target::LinuxX86_64));
    let windows = (Some(System::Windows), Some(Target::WindowsX86_64));
    assert!(fits_here(&[], &[], linux), "都没写的哪里都支持");
    assert!(fits_here(&[], &[], (None, None)));
    let unix = [System::Linux, System::Macos];
    assert!(fits_here(&unix, &[], linux));
    assert!(!fits_here(&unix, &[], windows));
    assert!(
        !fits_here(&unix, &[], (None, None)),
        "本机认不出的写了就不支持"
    );
    // 没写 systems 的照 targets：有本机这一种包文件才算。
    assert!(fits_here(&[], &[Target::LinuxX86_64], linux));
    assert!(!fits_here(&[], &[Target::LinuxAarch64], linux));
    assert!(!fits_here(&[], &[Target::LinuxX86_64], windows));
    // 认得出系统、认不出处理器的（Linux riscv）：只写 systems 的照支持，写了 targets 的不支持。
    assert!(fits_here(&unix, &[], (Some(System::Linux), None)));
    assert!(!fits_here(
        &unix,
        &[Target::LinuxX86_64],
        (Some(System::Linux), None)
    ));
}
