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

/// 给终端界面的语言总是中文（施工 MO-2、LW-32），不管宿主说什么：宿主的三个语言变量一个不接，换上
/// `LANG=zh_CN.UTF-8`；`PATH`、别的 `LC_*` 照接，其余的不接。发版的 Linux 机器是 `C.UTF-8`，没设语言的也一样。
#[test]
fn the_head_is_spoken_to_in_chinese_whatever_the_host_speaks() {
    let wanted = vec![
        (os("PATH"), os("/usr/bin")),
        (os("LC_CTYPE"), os("UTF-8")),
        (os("LANG"), os("zh_CN.UTF-8")),
    ];
    let english = vec![
        (os("PATH"), os("/usr/bin")),
        (os("LC_ALL"), os("en_US.UTF-8")),
        (os("LC_MESSAGES"), os("en_US.UTF-8")),
        (os("LC_CTYPE"), os("UTF-8")),
        (os("LANG"), os("C.UTF-8")),
        (os("HOME"), os("/home/someone")),
    ];
    assert_eq!(inherited(english.into_iter()), wanted);
    let unset = vec![(os("PATH"), os("/usr/bin")), (os("LC_CTYPE"), os("UTF-8"))];
    assert_eq!(inherited(unset.into_iter()), wanted);
    let chinese = vec![
        (os("PATH"), os("/usr/bin")),
        (os("LC_CTYPE"), os("UTF-8")),
        (os("LANG"), os("zh_CN.UTF-8")),
    ];
    assert_eq!(inherited(chinese.into_iter()), wanted);
}
