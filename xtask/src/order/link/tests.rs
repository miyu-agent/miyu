//! 垫片里不调链接器、不跑 `nm` 的几样：认输出名、认输入、读响应文件、报告的读写。参数照一台 M1 MacBook 上 rustc 链核心
//! 时真给 `cc` 的那一份抄（2026-11-23，前置任务记下的），路径换短了。

use super::*;

/// rustc 链核心时给 `cc` 的参数（中间的系统库删掉了几个）。
fn core_args() -> Vec<String> {
    [
        "/w/target/release/deps/miyu-1269a927180401de.miyu.83d5e61c9971cbc0-cgu.0.rcgu.o",
        "/w/target/release/deps/rustcypPgvm/libzstd_sys-84549f5b85d9ba0d.rlib",
        "/w/target/release/deps/rustcypPgvm/libring-b318f03982e830e6.rlib",
        "/w/target/release/deps/rustcypPgvm/liblibsqlite3_sys-e22384acba2d89c8.rlib",
        "/w/rustup/toolchains/1.97.0-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/lib/libcompiler_builtins-4d10940f69b776e6.rlib",
        "-framework",
        "Security",
        "-liconv",
        "-lSystem",
        "-arch",
        "arm64",
        "-mmacosx-version-min=11.0.0",
        "-L",
        "/w/target/release/build/ring-81e600d89f5e990b/out",
        "-o",
        "/w/target/release/deps/miyu-1269a927180401de",
        "-Wl,-dead_strip",
        "-nodefaultlibs",
    ]
    .map(String::from)
    .to_vec()
}

#[test]
fn the_output_follows_dash_o() {
    let args = core_args();
    assert_eq!(
        output(&args),
        Some(Path::new("/w/target/release/deps/miyu-1269a927180401de"))
    );
    assert_eq!(output(&["-o".to_string()]), None, "-o 后面什么都没有");
    assert_eq!(output(&[]), None);
}

#[cfg(unix)]
#[test]
fn only_the_core_and_the_tui_in_deps_are_ordered() {
    for out in [
        "/w/target/release/deps/miyu-1269a927180401de",
        "/w/tui/target/release/deps/miyu_tui-0123456789abcdef",
    ] {
        assert!(wanted(Path::new(out)), "{out}");
    }
    for out in [
        // 另外两个程序、构建脚本、过程宏原样交给 cc。
        "/w/target/release/deps/miyu_sandbox-1269a927180401de",
        "/w/target/release/deps/miyu_extract-1269a927180401de",
        "/w/target/release/build/ring-81e600d89f5e990b/build_script_build-81e600d89f5e990b",
        "/w/target/release/deps/libclap_derive-0123456789abcdef.dylib",
        // 名字对、地方或者哈希不对的也不排。
        "/w/target/release/miyu",
        "/w/target/release/build/miyu-1269a927180401de",
        "/w/target/release/deps/miyu-1269a927180401d",
        "/w/target/release/deps/miyu-1269a927180401dz",
        "/w/target/release/deps/miyu-perf-1269a927180401de",
    ] {
        assert!(!wanted(Path::new(out)), "{out}");
    }
}

#[cfg(unix)]
#[test]
fn inputs_are_the_objects_and_archives_under_deps() {
    let args = core_args();
    let out = output(&args).unwrap().to_path_buf();
    assert_eq!(
        inputs(&args, &out),
        [
            "/w/target/release/deps/miyu-1269a927180401de.miyu.83d5e61c9971cbc0-cgu.0.rcgu.o",
            "/w/target/release/deps/rustcypPgvm/libzstd_sys-84549f5b85d9ba0d.rlib",
            "/w/target/release/deps/rustcypPgvm/libring-b318f03982e830e6.rlib",
            "/w/target/release/deps/rustcypPgvm/liblibsqlite3_sys-e22384acba2d89c8.rlib",
        ],
        "工具链自带的 compiler_builtins、-L 的目录、选项都不算"
    );
}

#[test]
fn a_response_file_is_one_argument_per_line_with_backslash_escapes() {
    let text = "-o\n/w/with\\ space/deps/miyu-1269a927180401de\nback\\\\slash\n\n-lSystem\n";
    assert_eq!(
        response(text),
        [
            "-o",
            "/w/with space/deps/miyu-1269a927180401de",
            "back\\slash",
            "-lSystem",
        ]
    );
}

#[test]
fn response_files_are_expanded_in_place() {
    let dir = std::env::temp_dir().join(format!("xtask-order-link-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("linker-arguments");
    fs::write(&file, "-o\n/w/deps/miyu-1269a927180401de\n").unwrap();
    let args = [
        "-arch".to_string(),
        format!("@{}", file.display()),
        "-nodefaultlibs".to_string(),
    ];
    let expanded = expand(&args).unwrap();
    assert_eq!(
        expanded,
        [
            "-arch",
            "-o",
            "/w/deps/miyu-1269a927180401de",
            "-nodefaultlibs"
        ]
    );
    let missing = expand(&[format!("@{}", dir.join("nowhere").display())]).unwrap_err();
    assert!(missing.starts_with("响应文件 "), "{missing}");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_report_reads_back_what_was_written() {
    let report = Report {
        listed: 10622,
        found: 10501,
        symbols: 11203,
        millis: 412,
        order: PathBuf::from("/w/target/order/macos/miyu.order"),
    };
    assert_eq!(Report::parse(&report.to_json()).unwrap(), report);
    assert!(Report::parse("{}").unwrap_err().contains("listed"));
    assert!(Report::parse("not json").is_err());
}
