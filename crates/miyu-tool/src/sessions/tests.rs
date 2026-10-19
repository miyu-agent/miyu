//! 认会话编号（`cross-session.md`「对外的样子」会话的短编号）：整个编号、至少 8 位的后缀；别的写法对不上；撞了的说不止一个。

use super::*;

fn id(text: &str) -> SessionId {
    SessionId::parse(text).expect("合写法")
}

fn two() -> [SessionId; 2] {
    [
        id("0192f3a0-1111-7abc-8def-001122334455"),
        id("0192f3a0-2222-7abc-8def-5566778899aa"),
    ]
}

#[test]
fn the_whole_id_or_a_suffix_of_eight_or_more_finds_one() {
    let [a, b] = two();
    let among = [a.clone(), b.clone()];
    assert_eq!(find_session(a.as_str(), &among), Found::One(a.clone()));
    assert_eq!(find_session("22334455", &among), Found::One(a.clone()));
    assert_eq!(find_session("001122334455", &among), Found::One(a));
    assert_eq!(find_session("778899aa", &among), Found::One(b));
}

#[test]
fn other_ways_of_writing_find_nothing() {
    let among = two();
    for written in [
        "2334455",
        "778899AA",
        " 22334455",
        "22334455 ",
        "8def-001122334455",
        "",
        "0192f3a0",
        "0192F3A0-1111-7ABC-8DEF-001122334455",
    ] {
        assert_eq!(find_session(written, &among), Found::None, "{written:?}");
    }
}

#[test]
fn a_suffix_two_sessions_share_is_ambiguous_and_a_longer_one_is_not() {
    let a = id("0192f3a0-1111-7abc-8def-aaaa22334455");
    let b = id("0192f3a0-2222-7abc-8def-bbbb22334455");
    let among = [a.clone(), b];
    assert_eq!(find_session("22334455", &among), Found::Many);
    assert_eq!(find_session("aaaa22334455", &among), Found::One(a));
    assert_eq!(
        find_session("22334455", std::iter::empty()),
        Found::None,
        "一个都没有"
    );
}

#[test]
fn projects_wall_off_sessions_unless_both_sides_are_open() {
    let outside = (None, false);
    let closed = (Some("p1"), false);
    let open = (Some("p1"), true);
    let other_open = (Some("p2"), true);
    let other_closed = (Some("p2"), false);
    assert!(reaches(outside, outside), "都不在项目里");
    assert!(reaches(closed, closed), "同一个项目");
    assert!(
        !reaches(closed, outside) && !reaches(outside, closed),
        "项目没开的和外面互相看不到"
    );
    assert!(
        reaches(open, outside) && reaches(outside, open),
        "开了的和外面互相看得到"
    );
    assert!(reaches(open, other_open), "两个开了的项目互相看得到");
    assert!(
        !reaches(open, other_closed) && !reaches(other_closed, open),
        "另一边没开的看不到"
    );
}
