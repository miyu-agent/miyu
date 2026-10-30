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
        hint.contains("--no-sandbox")
            && hint.contains("--user-data-dir")
            && hint.contains("XDG_CONFIG_HOME"),
        "{hint}"
    );
    for said in [
        "[1022/231527.983720:ERROR:chrome/app/chrome_main.cc:210] Failed to create a unique user data directory for headless.\n",
        "[365480:365480:1022/231536.651620:FATAL:sandbox/linux/suid/client/setuid_sandbox_host.cc:172] The SUID sandbox helper binary was found, but is not configured correctly. Rather than run without sandboxing I'm aborting now.\n",
        "[376647:376647:1022/231643.615465:FATAL:content/browser/zygote_host/zygote_host_impl_linux.cc:213] Zygote process exited prematurely with exit code 1\n",
        "[1:1:FATAL:zygote_host_impl_linux.cc(127)] No usable sandbox! Update your kernel or see https://chromium.googlesource.com/chromium/src/+/main/docs/linux/suid_sandbox_development.md\n",
        // 加了 --no-sandbox --user-data-dir 以后，crashpad 在只读、被藏起来的 ~/.config/chromium 下建不了库（施工 SB-1 修二，
        // 主人验收撞见）：两种说法都认，后一句随系统语言变的那半不认。
        "chrome_crashpad_handler: --database is required\n",
        "[1023/180340.607686:ERROR:third_party/crashpad/crashpad/util/file/filesystem_posix.cc:63] mkdir /home/a/.config/chromium/Crash Reports/new: Permission denied (13)\n[1023/180340.607795:ERROR:third_party/crashpad/crashpad/util/linux/socket.cc:120] recvmsg: Connection reset by peer (104)\n",
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

/// firefox 截图写相对路径（施工 SB-1 修，主人验收撞见）：firefox 起来先切到 `/`，相对路径悄悄写不出来、退出码照样 0。命令里
/// 是 firefox、`--screenshot` 的路径是相对的（没写路径的默认 `screenshot.png` 也是）就在末尾接一句；绝对路径、`$PWD/`、`~/`
/// 开头的、chromium 的（它照工作目录写）不接。
#[test]
fn firefox_s_relative_screenshot_paths_get_a_hint() {
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let results = Results::load(&resources).expect("读得出");
    let hint = say(&results.screenshot, &[]);
    assert!(hint.contains("absolute"), "{hint}");
    let page = "https://example.com";
    for (command, wanted) in [
        (
            format!("firefox --headless --screenshot out.png {page}"),
            true,
        ),
        (
            format!("firefox --headless --screenshot=shots/out.png {page}"),
            true,
        ),
        (
            format!("firefox --headless --screenshot './out.png' {page}"),
            true,
        ),
        (
            format!("/usr/lib/firefox/firefox --headless --screenshot {page}"),
            true,
        ),
        (
            format!("cd /tmp && firefox-esr --screenshot out.png {page}"),
            true,
        ),
        (
            format!("firefox --headless --screenshot \"$PWD/out.png\" {page}"),
            false,
        ),
        (
            format!("firefox --headless --screenshot /tmp/out.png {page}"),
            false,
        ),
        (
            format!("firefox --headless --screenshot=~/out.png {page}"),
            false,
        ),
        (
            format!("chromium --headless --screenshot=out.png {page}"),
            false,
        ),
        (format!("firefox --headless {page}"), false),
    ] {
        let done = results.ended(
            Finished {
                ending: exited(0),
                output: captured("*** You are running in headless mode.\n"),
            },
            || unreachable!("没到时"),
        );
        let text = text_of(&results.screenshot(&command, done));
        assert_eq!(
            text.trim_end().ends_with(hint.trim_end()),
            wanted,
            "{command}\n{text}"
        );
    }
}

/// Windows 上沙盒里用 WMI 的命令（施工 SW-3，Windows 的容器挡住 WMI）：末尾接一句，她直接申请在沙盒外跑。不看退出码（施工 SW-4：
/// `Get-CimInstance` 连不上时照样退 0）；不在沙盒里的、不是 Windows 的、不用 WMI 的不接；别名、带路径和 `.exe` 的、管道后面的
/// 也认。
#[test]
fn a_failed_wmi_command_in_the_sandbox_gets_a_hint() {
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let results = Results::load(&resources).expect("读得出");
    let hint = say(&results.wmi, &[]);
    assert!(hint.contains("WMI"), "{hint}");
    let ended = |code| {
        results.ended(
            Finished {
                ending: exited(code),
                output: captured("Get-CimInstance : Access denied\n"),
            },
            || unreachable!("没到时"),
        )
    };
    for (command, code, sandboxed, windows, wanted) in [
        ("Get-CimInstance Win32_OperatingSystem", 1, true, true, true),
        (
            "gcim win32_bios | Select-Object Version",
            1,
            true,
            true,
            true,
        ),
        (
            "C:\\Windows\\System32\\wbem\\WMIC.exe os get caption",
            1,
            true,
            true,
            true,
        ),
        ("Get-Date; systeminfo /FO LIST", 1, true, true, true),
        ("(Get-ComputerInfo).OsName", 1, true, true, true),
        ("Get-CimInstance Win32_OperatingSystem", 0, true, true, true),
        (
            "Get-CimInstance Win32_OperatingSystem",
            1,
            false,
            true,
            false,
        ),
        (
            "Get-CimInstance Win32_OperatingSystem",
            1,
            true,
            false,
            false,
        ),
        ("Get-ChildItem C:\\nope", 1, true, true, false),
        ("Get-Content wmic-notes.txt", 1, true, true, false),
    ] {
        let text = text_of(&results.wmi(command, sandboxed, windows, ended(code)));
        assert_eq!(
            text.trim_end().ends_with(hint.trim_end()),
            wanted,
            "{command}\n{text}"
        );
    }
}
