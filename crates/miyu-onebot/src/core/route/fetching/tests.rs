//! `fetch_media` 的参数和一轮几次（施工 O-33）：`msg` 是整数或者整数的字，别的当没有这一条；`index` 不写是 1，写错的是 0（取的
//! 时候答「没有这一样」）；一轮最多几次，换了一轮从头数，主线没在跑的也数。取到的那一条要在这个会话里（施工 O-35）：群里是这个
//! 群的，私聊里是这个人在私聊里发的。

use serde_json::json;

use super::*;

fn want(msg: &str, index: usize) -> Result<Wanted, &'static str> {
    Ok(Wanted {
        msg: msg.to_string(),
        index,
    })
}

#[test]
fn msg_is_an_integer_and_index_counts_from_one() {
    assert_eq!(wanted(&json!({"msg": "8815"})), want("8815", 1));
    assert_eq!(wanted(&json!({"msg": 8815, "index": 2})), want("8815", 2));
    assert_eq!(
        wanted(&json!({"msg": "-2147483000", "index": "3"})),
        want("-2147483000", 3)
    );
    for index in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!("two"),
        json!([1]),
        json!(true),
    ] {
        assert_eq!(
            wanted(&json!({"msg": "1", "index": index.clone()})),
            want("1", 0),
            "{index}"
        );
    }
}

#[test]
fn a_msg_that_is_not_an_integer_is_not_found() {
    for msg in [
        json!(null),
        json!(""),
        json!("-"),
        json!("../8815"),
        json!("8815 "),
        json!("msg=8815"),
        json!("123456789012345678901"),
        json!(1.5),
        json!(u64::MAX),
        json!({"id": 1}),
    ] {
        assert_eq!(
            wanted(&json!({"msg": msg.clone()})),
            Err("not-found"),
            "{msg}"
        );
    }
    assert_eq!(wanted(&json!({})), Err("not-found"));
}

#[test]
fn a_turn_holds_at_most_the_limit_and_a_new_turn_starts_over() {
    let mut kept = None;
    for n in 1..=4 {
        kept = counted(kept, Some(7), 4);
        assert_eq!(kept, Some((Some(7), n)));
    }
    assert_eq!(counted(kept, Some(7), 4), None, "第五次");
    assert_eq!(counted(kept, Some(9), 4), Some((Some(9), 1)), "换了一轮");
    assert_eq!(counted(None, None, 4), Some((None, 1)), "主线没在跑的也数");
    assert_eq!(counted(Some((None, 4)), None, 4), None);
    assert_eq!(counted(None, Some(1), 0), None, "一次都不给");
}

/// 核对归属（施工 O-35）：群里只认这个群的（号写成字也认）；私聊只认 `message_type` 是 `private`、`user_id` 是这个人的，这个人在群里
/// 说的、别人的私聊、她自己发的都不认。
#[test]
fn only_a_message_from_this_venue_is_owned() {
    let group = To::Group(31);
    assert!(owned(
        &json!({"message_type": "group", "group_id": 31}),
        group
    ));
    assert!(owned(
        &json!({"message_type": "group", "group_id": "31"}),
        group
    ));
    for other in [
        json!({"message_type": "group", "group_id": 32}),
        json!({"message_type": "private", "user_id": 40001}),
        json!({}),
    ] {
        assert!(!owned(&other, group), "{other}");
    }
    let private = To::Private(40001);
    assert!(owned(
        &json!({"message_type": "private", "user_id": 40001}),
        private
    ));
    assert!(owned(
        &json!({"message_type": "private", "user_id": "40001"}),
        private
    ));
    for other in [
        json!({"message_type": "private", "user_id": 40002}),
        json!({"message_type": "group", "group_id": 31, "user_id": 40001}),
        json!({"user_id": 40001}),
        json!({"message_type": "private", "user_id": 30003}),
    ] {
        assert!(!owned(&other, private), "{other}");
    }
}
