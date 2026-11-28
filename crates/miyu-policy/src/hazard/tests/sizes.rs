//! 换变量换出来的字有上限（施工 AUD-7）：`x=$x$x` 这类赋值每一句都翻倍，不设上限时两百来字的命令就要十几 GB，判高危这一步把
//! 整个核心吃爆。超了的照看不透（[`Kind::TooDeep`]）问人。这几条要在限了内存的 cgroup 里跑，修之前是被系统杀掉的。

use super::*;

/// 十六个字的初值、九句各翻十倍：照字面换要 16 GB。看不透，照问人。
#[test]
fn doubling_assignments_are_too_deep_to_read() {
    let tenfold = "x=$x$x$x$x$x$x$x$x$x$x; ".repeat(9);
    for command in [
        format!("x=aaaaaaaaaaaaaaaa; {tenfold}echo done"),
        format!(
            "export x=aaaaaaaaaaaaaaaa; {}echo done",
            tenfold.replace("$x", "${x}")
        ),
    ] {
        assert_eq!(
            assess(&command, &PLACES),
            Some(Assessed::Asks(vec![Kind::TooDeep])),
            "{command}"
        );
    }
}

/// 一个词里引了很多次的（存下的值不长，换出来的长）也照看不透：删的目标照说不清问，也记一条看不透。
#[test]
fn a_word_expanding_past_the_limit_is_too_deep_to_read() {
    let value = "a".repeat(1024);
    let command = format!("x={value}; rm -rf {}", "$x".repeat(100));
    let Some(Assessed::Asks(kinds)) = assess(&command, &PLACES) else {
        panic!("{command}");
    };
    assert!(kinds.contains(&Kind::TooDeep), "{kinds:?}");
}

/// 平常的拼路径不受影响：换出来的照样认得。
#[test]
fn ordinary_assignments_still_expand() {
    assert_eq!(kind("T=/home/me; T=$T/; rm -rf $T"), Some("delete_home"));
    assert_eq!(kind("P=$HOME/proj; x=$P/build; rm -rf $x"), None);
}
