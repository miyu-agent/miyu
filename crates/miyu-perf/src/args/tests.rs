//! 参数：路径必写，数有默认，写错的说哪里不对。

use super::*;

fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_string).collect()
}

const PATHS_GIVEN: &str = "--miyu /m --resources /r --design /d --work /w --out /o";

#[test]
fn defaults_fill_the_numbers() {
    let args = parse(&words(PATHS_GIVEN)).unwrap();
    assert_eq!(args.miyu, PathBuf::from("/m"));
    assert_eq!(args.out, PathBuf::from("/o"));
    assert_eq!(
        (args.runs, args.sessions, args.events, args.tail),
        (10, 10, 10_000, 30)
    );
    assert_eq!(
        (args.reloads, args.appends, args.reply_bytes),
        (20, 2000, 3500)
    );
    assert_eq!(args.idle_wait, Duration::from_secs(200));
}

#[test]
fn numbers_can_be_given() {
    let args = parse(&words(&format!(
        "{PATHS_GIVEN} --runs 3 --sessions 4 --events 500 --tail 6 --reloads 2 --appends 50 --reply-bytes 100 --idle-wait 1"
    )))
    .unwrap();
    assert_eq!(
        (args.runs, args.sessions, args.events, args.tail),
        (3, 4, 500, 6)
    );
    assert_eq!((args.reloads, args.appends, args.reply_bytes), (2, 50, 100));
    assert_eq!(args.idle_wait, Duration::from_secs(1));
}

#[test]
fn rendering_needs_only_the_design_and_the_output() {
    let args = parse(&words("--render /o/x.json --design /d --out /o")).unwrap();
    assert_eq!(args.render, Some(PathBuf::from("/o/x.json")));
    let error = parse(&words("--render /o/x.json --out /o")).unwrap_err();
    assert!(error.starts_with("缺 --design\n"), "{error}");
    let error = parse(&words("--design /d --out /o")).unwrap_err();
    assert!(error.starts_with("缺 --miyu\n"), "{error}");
}

#[test]
fn mistakes_say_what_is_wrong() {
    let error = parse(&words("--miyu /m --resources /r --design /d --work /w")).unwrap_err();
    assert!(error.starts_with("缺 --out\n"), "{error}");
    let error = parse(&words(&format!("{PATHS_GIVEN} --runs -1"))).unwrap_err();
    assert!(
        error.starts_with("--runs 要一个非负整数，收到「-1」"),
        "{error}"
    );
    let error = parse(&words(&format!("{PATHS_GIVEN} --fast 1"))).unwrap_err();
    assert!(error.starts_with("不认识 --fast"), "{error}");
    let error = parse(&words(&format!("{PATHS_GIVEN} --runs"))).unwrap_err();
    assert!(error.starts_with("--runs 后面要跟一个值"), "{error}");
}

#[test]
fn the_gate_is_a_positive_factor() {
    assert_eq!(parse(&words(PATHS_GIVEN)).unwrap().gate, None);
    let args = parse(&words(&format!("{PATHS_GIVEN} --gate 1.5"))).unwrap();
    assert_eq!(args.gate, Some(1.5));
    for wrong in ["0", "-1", "x", "inf"] {
        let error = parse(&words(&format!("{PATHS_GIVEN} --gate {wrong}"))).unwrap_err();
        assert!(
            error.starts_with(&format!("--gate 要一个正数，收到「{wrong}」")),
            "{error}"
        );
    }
}

/// 工具一步（施工 V-29）：`--tools` 要和 `--tree` 一起写；日志级别、标签照写的。
#[test]
fn the_tool_step_needs_a_tree() {
    let args = parse(&words(PATHS_GIVEN)).unwrap();
    assert_eq!((args.tools, args.tree.clone()), (None, None));
    let args = parse(&words(&format!(
        "{PATHS_GIVEN} --tools 5 --tree /t --tools-log debug --tag 拆分"
    )))
    .unwrap();
    assert_eq!(args.tools, Some(5));
    assert_eq!(args.tree, Some(PathBuf::from("/t")));
    assert_eq!(args.tools_log.as_deref(), Some("debug"));
    assert_eq!(args.tag.as_deref(), Some("拆分"));
    assert!(
        parse(&words(&format!("{PATHS_GIVEN} --tools 5")))
            .unwrap_err()
            .contains("--tree")
    );
    assert!(parse(&words(&format!("{PATHS_GIVEN} --tools x --tree /t"))).is_err());
}

/// 排函数（施工 LW-1）：`--order`、`--emulator` 一起写，`--tree` 必写；预算、结果目录用不着。
#[test]
fn recording_the_order_needs_an_emulator_and_a_tree() {
    const ORDER: &str = "--miyu /m --resources /r --work /w --tree /t";
    let args = parse(&words(&format!(
        "{ORDER} --order /o.txt --emulator qemu-x86_64"
    )))
    .unwrap();
    assert_eq!(args.order, Some(PathBuf::from("/o.txt")));
    assert_eq!(args.emulator, Some(PathBuf::from("qemu-x86_64")));
    assert_eq!(args.tree, Some(PathBuf::from("/t")));
    assert_eq!(parse(&words(PATHS_GIVEN)).unwrap().order, None);
    for half in ["--order /o.txt", "--emulator qemu-x86_64"] {
        let error = parse(&words(&format!("{PATHS_GIVEN} {half}"))).unwrap_err();
        assert!(
            error.starts_with("--order 要和 --emulator 或者 --probe 一起写\n"),
            "{error}"
        );
    }
    let error = parse(&words(
        "--miyu /m --resources /r --work /w --order /o.txt --emulator qemu-x86_64",
    ))
    .unwrap_err();
    assert!(error.starts_with("缺 --tree\n"), "{error}");
    let error = parse(&words(
        "--miyu /m --work /w --tree /t --order /o.txt --emulator qemu-x86_64",
    ))
    .unwrap_err();
    assert!(error.starts_with("缺 --resources\n"), "{error}");
}

/// 排函数配着真的终端界面（施工 LW-6）：`--head-order` 要和 `--head` 一起写（只写 `--head` 的只录第三段），只在 `--order`
/// 的跑法里认；不写的和原来一样。
#[test]
fn the_head_goes_with_its_own_order_and_only_when_recording() {
    const ORDER: &str =
        "--miyu /m --resources /r --work /w --tree /t --order /o.txt --emulator qemu-x86_64";
    let args = parse(&words(ORDER)).unwrap();
    assert_eq!((args.head, args.head_order), (None, None));
    let args = parse(&words(&format!(
        "{ORDER} --head /d/miyu-tui --head-order /tui.txt"
    )))
    .unwrap();
    assert_eq!(args.head, Some(PathBuf::from("/d/miyu-tui")));
    assert_eq!(args.head_order, Some(PathBuf::from("/tui.txt")));
    let args = parse(&words(&format!("{ORDER} --head /d/miyu-tui"))).unwrap();
    assert_eq!(
        (args.head, args.head_order),
        (Some(PathBuf::from("/d/miyu-tui")), None)
    );
    let error = parse(&words(&format!("{ORDER} --head-order /tui.txt"))).unwrap_err();
    assert!(
        error.starts_with("--head-order 要和 --head 一起写\n"),
        "{error}"
    );
    let error = parse(&words(&format!(
        "{PATHS_GIVEN} --head /d/miyu-tui --head-order /tui.txt"
    )))
    .unwrap_err();
    assert!(
        error.starts_with("--head 只在 --order 的跑法里认\n"),
        "{error}"
    );
}

/// 排函数带页探针（施工 MO-2）：`--probe` 代替 `--emulator`，两个只能写一个；只写 `--probe` 不写 `--order` 的说清楚；
/// 两个都不写的照原来的报错。
#[test]
fn the_probe_replaces_the_emulator() {
    const ORDER: &str = "--miyu /m --resources /r --work /w --tree /t --order /o.txt";
    let args = parse(&words(&format!(
        "{ORDER} --probe /p/pagetrace.dylib --head /d/miyu-tui --head-order /tui.txt"
    )))
    .unwrap();
    assert_eq!(args.probe, Some(PathBuf::from("/p/pagetrace.dylib")));
    assert_eq!(args.emulator, None);
    assert_eq!(args.head_order, Some(PathBuf::from("/tui.txt")));
    assert_eq!(parse(&words(PATHS_GIVEN)).unwrap().probe, None);
    let error = parse(&words(&format!(
        "{ORDER} --emulator qemu-x86_64 --probe /p/pagetrace.dylib"
    )))
    .unwrap_err();
    assert!(
        error.starts_with("--emulator 和 --probe 只能写一个："),
        "{error}"
    );
    let error = parse(&words(&format!("{PATHS_GIVEN} --probe /p/pagetrace.dylib"))).unwrap_err();
    assert!(
        error.starts_with("--probe 要和 --order 一起写\n"),
        "{error}"
    );
    let error = parse(&words(ORDER)).unwrap_err();
    assert!(
        error.starts_with("--order 要和 --emulator 或者 --probe 一起写\n"),
        "{error}"
    );
    // 用法里写着两种录法。
    assert!(error.contains("--probe <pagetrace.dylib>"), "{error}");
    let error = parse(&words(
        "--miyu /m --resources /r --work /w --order /o.txt --probe /p/pagetrace.dylib",
    ))
    .unwrap_err();
    assert!(error.starts_with("缺 --tree\n"), "{error}");
}
