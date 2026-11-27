//! 用量 = 锚 + 锚以后新加的估算：锚盖住哪些、估算比报的大时取估算、没有能用的锚就整份估。

use super::*;

#[test]
fn the_usage_is_the_anchor_plus_what_came_after_its_reply() {
    // 锚那次请求一条消息，回复是第 1 条；之后多了一条 4 字节、一条 8 字节的。
    let now = request(
        "sys",
        vec![
            user("hello"),
            assistant("hi"),
            user("abcd"),
            user("abcdefgh"),
        ],
    );
    assert_eq!(
        usage(&now, Some(&anchored(1, 1000)), &flash(), &FLAT, Scale::ONE),
        1000 + 1 + 2
    );
    // 锚之后什么都没有。
    let now = request("sys", vec![user("hello"), assistant("hi")]);
    assert_eq!(
        usage(&now, Some(&anchored(1, 1000)), &flash(), &FLAT, Scale::ONE),
        1000
    );
}

#[test]
fn an_estimate_above_what_was_reported_wins() {
    // 锚盖住的：system 400 字节算 100，两条各 1，一共 102，比报的 50 大。
    let system = "x".repeat(400);
    let now = request(&system, vec![user("a"), assistant("b"), user("abcd")]);
    assert_eq!(
        usage(&now, Some(&anchored(1, 50)), &flash(), &FLAT, Scale::ONE),
        102 + 1
    );
}

#[test]
fn the_whole_request_is_estimated_without_a_usable_anchor() {
    let now = request("sys", vec![user("hello"), assistant("hi"), user("abcd")]);
    // 3 字节的 system 算 1，5 字节算 2，2 字节算 1，4 字节算 1。
    let whole = 1 + 2 + 1 + 1;
    assert_eq!(usage(&now, None, &flash(), &FLAT, Scale::ONE), whole);

    // 换过模型：锚是别的模型报的。
    let other = Model {
        endpoint: ProviderId::parse("deepseek").unwrap(),
        model: ModelName::parse("deepseek-v4-pro").unwrap(),
    };
    assert_eq!(
        usage(&now, Some(&anchored(1, 1000)), &other, &FLAT, Scale::ONE),
        whole
    );

    // 这一次的消息没比锚多：锚说有三条，回复该是第 3 条，可一共只有三条。
    assert_eq!(
        usage(&now, Some(&anchored(3, 1000)), &flash(), &FLAT, Scale::ONE),
        whole
    );
}
