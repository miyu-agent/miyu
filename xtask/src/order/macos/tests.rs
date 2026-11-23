//! macOS 排函数里不编、不链的几样：给 cargo 的 `--config`、垫片脚本、报告说成的话、指纹。

use super::*;

fn report(listed: usize, found: usize) -> Report {
    Report {
        listed,
        found,
        symbols: found + 700,
        millis: 412,
        order: PathBuf::from("/w/target/order/macos/miyu.order"),
    }
}

#[test]
fn the_report_is_one_line_when_most_names_match() {
    assert_eq!(
        describe("核心", &report(10622, 10501)),
        [
            "排函数：核心的起点清单 10622 个名字，对上 10501 个（98%），写出 11201 个符号，读符号表用了 0.41 秒"
        ]
    );
}

#[test]
fn fewer_than_half_matching_warns_that_the_seed_is_stale() {
    let lines = describe("终端界面", &report(2333, 1166));
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[1].contains("起点清单太旧"), "{lines:?}");
    // 正好一半不提醒。
    assert_eq!(describe("终端界面", &report(2334, 1167)).len(), 1);
    // 空清单不除以零（prepare 已经拦下了，这里只是不崩）。
    assert_eq!(describe("核心", &report(0, 0)).len(), 1);
}

#[test]
fn the_linker_config_is_a_toml_string() {
    assert_eq!(
        config(Path::new("/w/target/order/shim/0123456789abcdef/cc")).unwrap(),
        "target.aarch64-apple-darwin.linker=\"/w/target/order/shim/0123456789abcdef/cc\""
    );
    assert_eq!(
        config(Path::new("/w/a \"b\" \\c/cc")).unwrap(),
        "target.aarch64-apple-darwin.linker=\"/w/a \\\"b\\\" \\\\c/cc\""
    );
    assert!(config(Path::new("/w/a\nb/cc")).is_err());
}

#[test]
fn the_fingerprint_follows_the_seed_text() {
    let one = fingerprint("main\n");
    assert_eq!(one.len(), 16);
    assert!(one.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(one, fingerprint("main\n"));
    assert_ne!(one, fingerprint("main\nmiyu::main\n"));
}

#[test]
fn the_shim_passes_straight_to_cc_unless_names_are_set() {
    let script = shim_script("/w/it's/xtask");
    assert!(script.starts_with("#!/bin/sh\n"), "{script}");
    assert!(
        script
            .contains("if [ -z \"${MIYU_ORDER_NAMES:-}\" ]; then\n  exec /usr/bin/cc \"$@\"\nfi\n"),
        "{script}"
    );
    assert!(
        script.ends_with("exec '/w/it'\\''s/xtask' order-link \"$@\"\n"),
        "{script}"
    );
}

/// 真跑一遍垫片脚本：设了 `MIYU_ORDER_NAMES` 的交给 xtask，参数一个不差（带空格、带引号的也是）。xtask 换成一个把参数
/// 一行一个写下来的脚本，放在带单引号的目录里。
#[cfg(unix)]
#[test]
fn the_shim_hands_every_argument_to_order_link() {
    let dir = std::env::temp_dir().join(format!("xtask-order-shim-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    let fake = dir.join("it's").join("xtask");
    let seen = dir.join("seen.txt");
    make_parent(&fake).unwrap();
    fs::write(
        &fake,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\"; done > '{}'\n",
            seen.display()
        ),
    )
    .unwrap();
    executable(&fake).unwrap();
    let shim = dir.join("shim").join("cc");
    write_shim(&shim, fake.to_str().unwrap()).unwrap();
    let done = Command::new(&shim)
        .env(NAMES, "/w/seed.txt")
        .args(["-o", "/w/deps/miyu-1269a927180401de", "a b", "\"q\""])
        .status()
        .unwrap();
    assert!(done.success());
    assert_eq!(
        fs::read_to_string(&seen).unwrap(),
        "order-link\n-o\n/w/deps/miyu-1269a927180401de\na b\n\"q\"\n"
    );
    fs::remove_dir_all(&dir).unwrap();
}
