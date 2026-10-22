use super::output::Capture;
use super::process::Ending;
use super::*;

#[test]
fn the_timeout_is_kept_within_bounds() {
    assert_eq!(limit(None), DEFAULT);
    assert_eq!(limit(Some(0)), DEFAULT);
    assert_eq!(limit(Some(1)), 1);
    assert_eq!(limit(Some(MAX)), MAX);
    assert_eq!(limit(Some(MAX + 1)), MAX);
}

#[test]
fn seconds_drop_a_zero_fraction() {
    assert_eq!(seconds(120_000), "120");
    assert_eq!(seconds(300), "0.3");
    assert_eq!(seconds(1_500), "1.5");
}

#[test]
fn the_schema_states_the_same_limits() {
    let schema = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/software/basesystem/tools/shell.json"),
    )
    .expect("读得出");
    assert!(
        schema.contains(&format!("up to {MAX}. Default {DEFAULT}.")),
        "{schema}"
    );
}

#[test]
fn a_body_ends_each_piece_with_a_line_break() {
    assert_eq!(line(String::new()), "");
    assert_eq!(line("a".into()), "a\n");
    assert_eq!(line("a\n".into()), "a\n");
}

/// 一份跑完了的输出：标准输出、标准错误一起读到的 `text`。
fn captured(text: &str) -> Capture {
    let mut output = Capture::new(false);
    output.push(text.as_bytes());
    output.finished()
}

/// 退出码是 `code` 的结束。
fn exited(code: i32) -> Ending {
    #[cfg(unix)]
    let status = std::os::unix::process::ExitStatusExt::from_raw(code << 8);
    #[cfg(windows)]
    let status = std::os::windows::process::ExitStatusExt::from_raw(code.cast_unsigned());
    Ending::Exited(status)
}

/// chromium 自己那层沙盒、资料目录在沙盒里起不来（施工 SB-1，2026-10-23 项目主人定）：认出它自己的报错，结果末尾接一句教她
/// 加 `--no-sandbox --user-data-dir=<临时目录>`；别的失败、跑成了的不接。只有撞上才花 token，不进 `shell` 的说明。
#[test]
fn chromium_s_own_sandbox_failures_get_a_hint_and_nothing_else_does() {
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let results = Results::load(&resources).expect("读得出");
    let hint = say(&results.chromium, &[]);
    assert!(
        hint.contains("--no-sandbox") && hint.contains("--user-data-dir"),
        "{hint}"
    );
    for said in [
        "[1022/231527.983720:ERROR:chrome/app/chrome_main.cc:210] Failed to create a unique user data directory for headless.\n",
        "[365480:365480:1022/231536.651620:FATAL:sandbox/linux/suid/client/setuid_sandbox_host.cc:172] The SUID sandbox helper binary was found, but is not configured correctly. Rather than run without sandboxing I'm aborting now.\n",
        "[376647:376647:1022/231643.615465:FATAL:content/browser/zygote_host/zygote_host_impl_linux.cc:213] Zygote process exited prematurely with exit code 1\n",
        "[1:1:FATAL:zygote_host_impl_linux.cc(127)] No usable sandbox! Update your kernel or see https://chromium.googlesource.com/chromium/src/+/main/docs/linux/suid_sandbox_development.md\n",
    ] {
        let done = results.ended(
            Finished {
                ending: exited(1),
                output: captured(said),
            },
            || unreachable!("没到时"),
        );
        let text = text_of(&done);
        assert!(
            done.error && text.trim_end().ends_with(hint.trim_end()),
            "{text}"
        );
    }
    let other = results.ended(
        Finished {
            ending: exited(1),
            output: captured("ls: cannot access 'x'\n"),
        },
        || unreachable!("没到时"),
    );
    assert!(!text_of(&other).contains("--no-sandbox"));
    let fine = results.ended(
        Finished {
            ending: exited(0),
            output: captured("Failed to create a unique user data directory, but we went on\n"),
        },
        || unreachable!("没到时"),
    );
    assert!(!text_of(&fine).contains("--no-sandbox"), "跑成了的不接");
}

/// 交回的字。
fn text_of(done: &Done) -> String {
    done.blocks
        .iter()
        .filter_map(|block| match block {
            miyu_kernel::block::Block::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}
