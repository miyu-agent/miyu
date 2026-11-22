//! 配着真的终端界面录的两段：看主程序怎么拉起终端界面的几样纯函数，和等回答认的那段字。

use super::*;

fn os(text: &str) -> OsString {
    OsString::from(text)
}

#[test]
fn proc_lists_drop_only_the_last_nul() {
    assert_eq!(nul_list(b"/d/miyu-tui\0"), vec![os("/d/miyu-tui")]);
    assert_eq!(
        nul_list(b"/d/miyu-tui\0--page\0config\0"),
        vec![os("/d/miyu-tui"), os("--page"), os("config")]
    );
    // 空的参数是真的参数，照样留着。
    assert_eq!(
        nul_list(b"/d/miyu-tui\0\0"),
        vec![os("/d/miyu-tui"), os("")]
    );
    assert!(nul_list(b"").is_empty());
}

#[test]
fn only_what_the_launcher_added_or_changed_is_extra() {
    let given = vec![
        (os("HOME"), os("/w/user")),
        (os("TERM"), os("xterm-256color")),
    ];
    let environ = vec![
        os("HOME=/w/user"),
        os("TERM=xterm"),
        os("MIYU_HEAD=tui"),
        os("WEIRD"),
        os("EQUALS=a=b"),
    ];
    assert_eq!(
        extra(&environ, &given),
        vec![
            (os("TERM"), os("xterm")),
            (os("MIYU_HEAD"), os("tui")),
            (os("EQUALS"), os("a=b")),
        ]
    );
    assert!(extra(&[os("HOME=/w/user")], &given).is_empty());
}

/// 屏幕上认回答的那段字要真在假模型的回答里，回答多长都在（量尺默认 3500 字节，最短的一段也带着它）。
#[test]
fn the_answer_mark_is_in_every_reply() {
    for bytes in [1, 3500] {
        assert!(crate::fake::filler(bytes).contains(ANSWERED), "{bytes}");
    }
}

/// 带页探针的（施工 MO-2）把终端界面的语言换成中文：去掉宿主的三个语言变量，换上 `LANG=zh_CN.UTF-8`，别的不动。
#[test]
fn the_probe_path_speaks_chinese_to_the_head() {
    let mut vars = vec![
        (os("PATH"), os("/usr/bin")),
        (os("LC_ALL"), os("en_US.UTF-8")),
        (os("LC_MESSAGES"), os("en_US.UTF-8")),
        (os("LC_CTYPE"), os("UTF-8")),
        (os("LANG"), os("en_US.UTF-8")),
    ];
    chinese(&mut vars);
    assert_eq!(
        vars,
        vec![
            (os("PATH"), os("/usr/bin")),
            (os("LC_CTYPE"), os("UTF-8")),
            (os("LANG"), os("zh_CN.UTF-8")),
        ]
    );
}
