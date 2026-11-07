//! 同一个请求编出了不同的字节（施工 MD-3）：只说一次；一样的不说；别的驱动各说各的；只留最近几次。

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
