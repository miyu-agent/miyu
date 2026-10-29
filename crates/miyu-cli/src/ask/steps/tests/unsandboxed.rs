//! 沙盒用不了的告警（施工 5-4 下，设计 47）：每种原因照这台机器的系统写成人话，能修的另起一行提示。

use crate::diagnostic::Level;
use crate::language::Language;

/// 沙盒用不了那一句（施工 5-4 下）：每种原因照这台机器的系统写成人话；中英两种；不认得的原因照原样写进括号。
#[test]
fn an_unusable_sandbox_is_said_with_why_and_how_to_fix() {
    use miyu_sandbox::Platform;

    let every = [
        Platform::Linux,
        Platform::Macos,
        Platform::Windows,
        Platform::Other,
    ];
    // 照设计 47 写成告警：为什么写在 `miyu: 警告：` 那一行，能修的另起一行提示。
    // 原因、在哪几种系统上、中文的几行、英文的几行。
    type Case<'a> = (&'a str, &'a [Platform], &'a [&'a str], &'a [&'a str]);
    let cases: [Case<'_>; 6] = [
        (
            "helper_missing",
            &every,
            &[
                "miyu: 警告：沙盒不可用：主程序旁边没有 miyu-sandbox",
                "提示：重新安装 Miyu",
            ],
            &[
                "miyu: warning: sandbox unavailable: miyu-sandbox is missing beside the main program",
                "hint: reinstall Miyu",
            ],
        ),
        (
            "helper_failed",
            &every,
            &[
                "miyu: 警告：沙盒不可用：miyu-sandbox 无法运行",
                "提示：重新安装 Miyu",
            ],
            &[
                "miyu: warning: sandbox unavailable: miyu-sandbox does not run",
                "hint: reinstall Miyu",
            ],
        ),
        (
            "no_mechanism",
            &[Platform::Linux],
            &[
                "miyu: 警告：沙盒不可用：内核没有可用的 Landlock",
                "提示：需要 Linux 5.13 或更新，并在启动参数 lsm= 里启用 Landlock",
            ],
            &[
                "miyu: warning: sandbox unavailable: the kernel has no usable Landlock",
                "hint: use Linux 5.13 or later with Landlock enabled in the lsm= boot parameter",
            ],
        ),
        (
            "no_mechanism",
            &[Platform::Macos],
            &["miyu: 警告：沙盒不可用：无法应用 Seatbelt 配置，Miyu 可能运行在另一个沙盒里"],
            &[
                "miyu: warning: sandbox unavailable: the Seatbelt profile cannot be applied; Miyu may be running inside another sandbox",
            ],
        ),
        (
            "no_mechanism",
            &[Platform::Windows],
            &["miyu: 警告：沙盒不可用：此版本在 Windows 上还不能把命令关进沙盒"],
            &[
                "miyu: warning: sandbox unavailable: this version cannot sandbox commands on Windows yet",
            ],
        ),
        (
            "no_mechanism",
            &[Platform::Other],
            &["miyu: 警告：沙盒不可用：此系统上没有可用的沙盒"],
            &["miyu: warning: sandbox unavailable: no sandbox is available on this system"],
        ),
    ];
    for (reason, platforms, chinese, english) in cases {
        for &platform in platforms {
            assert_eq!(
                Language::Chinese
                    .unsandboxed(reason, platform)
                    .lines(Language::Chinese, Level::Warning),
                chinese,
                "{reason} {platform:?}"
            );
            assert_eq!(
                Language::English
                    .unsandboxed(reason, platform)
                    .lines(Language::English, Level::Warning),
                english,
                "{reason} {platform:?}"
            );
        }
    }
    assert_eq!(
        Language::Chinese
            .unsandboxed("no_user\u{1b}[2J", Platform::Windows)
            .lines(Language::Chinese, Level::Warning),
        ["miyu: 警告：沙盒不可用：no_user\u{fffd}[2J"],
        "不认得的原因照原样写，控制字符换掉"
    );
}
