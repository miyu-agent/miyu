//! 调用的事实（施工 X-43）：本机的会话一直是主人；场所会话照这一轮开口的人，没人开口的、几个人里有一个不是的不是；别的一轮的、
//! 没记下的不是。

use miyu_kernel::id::{ExternalId, Seq};

use super::*;

fn speaker(id: &str, owner: bool) -> Speaker {
    Speaker {
        id: ExternalId::parse(id).expect("合写法"),
        owner,
    }
}

fn turn(seq: u64) -> TurnId {
    TurnId::new(Seq::new(seq).expect("序号合写法"))
}

fn callers(venue: &str) -> Callers {
    Callers::new(VenueId::parse(venue).expect("合写法"))
}

#[test]
fn a_venue_round_is_the_owners_only_when_every_speaker_is() {
    for (speakers, owner) in [
        (vec![speaker("qq:1", true)], true),
        (vec![speaker("qq:1", true), speaker("qq:2", true)], true),
        (vec![speaker("qq:1", true), speaker("qq:2", false)], false),
        (vec![speaker("qq:2", false)], false),
        (Vec::new(), false),
    ] {
        let mut group = callers("qq:group:1");
        group.start(turn(5), &speakers);
        let caller = group.of(Some(turn(5)));
        assert_eq!(caller.owner, owner, "{speakers:?}");
        assert_eq!(caller.venue.as_str(), "qq:group:1");
    }
}

#[test]
fn another_round_or_none_recorded_is_not_the_owners() {
    let mut group = callers("qq:group:1");
    assert!(!group.of(Some(turn(5))).owner, "没记下的");
    group.start(turn(5), &[speaker("qq:1", true)]);
    assert!(!group.of(Some(turn(9))).owner, "别的一轮");
    assert!(!group.of(None).owner, "不在哪一轮的");
}

#[test]
fn a_local_session_is_always_the_owners() {
    let local = callers("local");
    assert!(local.of(None).owner);
    let mut local = callers("local");
    local.start(turn(5), &[]);
    assert!(local.of(Some(turn(5))).owner);
}
