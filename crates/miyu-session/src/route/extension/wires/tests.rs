//! 同一个请求编出了不同的字节（施工 MD-3）：只说一次；一样的不说；别的驱动各说各的；只留最近几次。没声明的供应商侧工具
//! （施工 HT-4）：每个名字说一次，每个驱动最多列 [`UNDECLARED_PER_DRIVER`] 个，满了的、太长的只再说一次「还有更多」。

use super::*;

fn hash(text: &str) -> ContentHash {
    ContentHash::of(text.as_bytes())
}

#[test]
fn a_different_wire_for_the_same_request_is_told_once_per_driver() {
    let mut wires = Wires::default();
    let (request, first, second) = (hash("r"), hash("a"), hash("b"));
    assert_eq!(wires.note("pi/ag", &request, &first), None, "头一次");
    assert_eq!(wires.note("pi/ag", &request, &first), None, "一样的不说");
    assert_eq!(
        wires.note("pi/ag", &request, &second),
        Some(first.clone()),
        "不一样的说"
    );
    assert_eq!(
        wires.note("pi/ag", &request, &hash("c")),
        None,
        "一次运行只说一次"
    );
    assert_eq!(wires.note("other/ag", &request, &first), None);
    assert_eq!(
        wires.note("other/ag", &request, &second),
        Some(first),
        "别的驱动各说各的"
    );
}

#[test]
fn only_the_last_few_requests_are_kept() {
    let mut wires = Wires::default();
    let old = hash("old");
    wires.note("pi/ag", &old, &hash("a"));
    for n in 0..KEPT {
        wires.note("pi/ag", &hash(&n.to_string()), &hash("x"));
    }
    assert_eq!(
        wires.note("pi/ag", &old, &hash("b")),
        None,
        "挤出去了的不再比"
    );
}

/// 第 `n` 个没声明的名字。
fn name(n: usize) -> String {
    format!("tool_{n}")
}

#[test]
fn undeclared_names_are_told_once_up_to_the_cap_then_more_once() {
    let mut wires = Wires::default();
    for n in 0..UNDECLARED_PER_DRIVER {
        assert_eq!(
            wires.undeclared("pi/ag", &name(n)),
            Undeclared::Tell,
            "第 {n} 个进表"
        );
    }
    let over = name(UNDECLARED_PER_DRIVER);
    assert_eq!(
        wires.undeclared("pi/ag", &over),
        Undeclared::TellMore,
        "满了：说还有更多"
    );
    assert_eq!(
        wires.undeclared("pi/ag", &name(UNDECLARED_PER_DRIVER + 1)),
        Undeclared::ToldMore,
        "还有更多只说一次"
    );
    assert_eq!(
        wires.undeclared("pi/ag", &over),
        Undeclared::ToldMore,
        "没进表的再来也不说"
    );
    for n in 0..UNDECLARED_PER_DRIVER {
        assert_eq!(
            wires.undeclared("pi/ag", &name(n)),
            Undeclared::Told,
            "第 {n} 个说过了"
        );
    }
}

#[test]
fn a_too_long_name_takes_no_place_and_tells_more_once() {
    let mut wires = Wires::default();
    let longest = "x".repeat(HOSTED_NAME_BYTES);
    let long = "x".repeat(HOSTED_NAME_BYTES + 1);
    assert_eq!(
        wires.undeclared("pi/ag", &long),
        Undeclared::TellMore,
        "太长的不进表"
    );
    assert_eq!(
        wires.undeclared("pi/ag", &long),
        Undeclared::ToldMore,
        "再来不说"
    );
    assert_eq!(
        wires.undeclared("pi/ag", &longest),
        Undeclared::Tell,
        "正好上限的进表"
    );
    for n in 1..UNDECLARED_PER_DRIVER {
        assert_eq!(
            wires.undeclared("pi/ag", &name(n)),
            Undeclared::Tell,
            "太长的不占名额：第 {n} 个照样进表"
        );
    }
    assert_eq!(
        wires.undeclared("pi/ag", &name(0)),
        Undeclared::ToldMore,
        "满了，还有更多说过了"
    );
    assert_eq!(wires.undeclared("pi/ag", &longest), Undeclared::Told);
}

#[test]
fn each_driver_has_its_own_undeclared_cap() {
    let mut wires = Wires::default();
    for n in 0..=UNDECLARED_PER_DRIVER {
        wires.undeclared("pi/ag", &name(n));
    }
    assert_eq!(
        wires.undeclared("pi/ag", &name(UNDECLARED_PER_DRIVER + 1)),
        Undeclared::ToldMore
    );
    assert_eq!(
        wires.undeclared("other/ag", &name(0)),
        Undeclared::Tell,
        "别的驱动的表没满"
    );
    assert_eq!(
        wires.undeclared("other/ag", &"x".repeat(HOSTED_NAME_BYTES + 1)),
        Undeclared::TellMore,
        "还有更多各说各的"
    );
    assert_eq!(wires.undeclared("pi/ag", &name(0)), Undeclared::Told);
}
