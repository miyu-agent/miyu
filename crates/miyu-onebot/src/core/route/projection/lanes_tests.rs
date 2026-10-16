//! 主线投影里的支线（施工 O-36，`onebot.md` 第一条「群里怎么叫她」第 2 条「支线」）：支线的序号换回主线序号；分派要的 `Lines`；
//! 开轮带的事实指哪几条；两条线的回合编号撞上时这一轮发出去的、她的最后一段、判过要回的不串线；接着开的一轮照支线自己并进来的
//! 找回；抄进支线用的人话照抄的写法、留到用不着；封存了去掉。夹具在 `tests.rs`。

use miyu_chat::{Line, Lines};
use miyu_client::events::Event;
use serde_json::json;

use super::tests::{
    LINE, assistant, decided, ended, event, joined, pendings, qq, said_at, started, take_all,
};
use super::{Projection, QUEUED, Seen};

/// 支线的子会话编号。
const LANE: &str = "01a0d78c-ffff-7d19-8b64-0e3f5a7c2d91";

/// 号是 `user` 的人在群里说的第 `seq` 条，平台编号 `m<seq>`，命令编号 `c<seq>`，带一张图。
fn heard(seq: u64, user: i64) -> Event {
    let by = json!({"kind": "external", "venue": "qq:group:5", "id": format!("qq:{user}"), "role": "member"});
    let blob = format!("sha256:{}", "a".repeat(64));
    let image = json!({"type": "image", "blob": blob, "name": "msg-1-1", "media_type": "image/png", "width": 1, "height": 1});
    let body = json!({"blocks": [{"type": "text", "text": "嗨"}, image], "venue": {"msg": format!("m{seq}"), "ambient": true}});
    let mut line = json!({"seq": seq, "at": "2026-10-11T00:00:00.000Z", "kind": "message.user", "by": by, "cause": format!("c{seq}"), "body": body});
    line["body"]["venue"]["name"] = json!("某人");
    serde_json::from_value(line).expect("读得成事件")
}

/// 支线的第 `seq` 条：封存了。
fn sealed(seq: u64) -> Event {
    event(
        seq,
        0,
        "session.sealed",
        None,
        json!({"kind": "kernel"}),
        json!({}),
    )
}

/// 线 `line` 第 `turn` 轮发出去的一段。
fn delivered(seq: u64, line: &str, turn: u64) -> Event {
    let body =
        json!({"line": line, "turn": turn, "to": [], "msg": format!("d{seq}"), "text": "在。"});
    event(
        seq,
        0,
        "venue.delivered",
        None,
        json!({"kind": "module", "id": "onebot"}),
        body,
    )
}

/// 线 `line` 第 `turn` 轮入队了的一段 `text`。
fn queued(seq: u64, line: &str, turn: u64, text: &str) -> Event {
    let body = json!({"kind": "reply", "text": text, "line": line, "turn": turn});
    event(
        seq,
        0,
        QUEUED,
        None,
        json!({"kind": "module", "id": "onebot"}),
        body,
    )
}

/// 主线：A（20002）说第 1 条、第 2 条开一轮回他；B（20003）说第 3 条；分叉点是 1，分派给支线的是第 3 条。
fn forked() -> Projection {
    let mut projection = Projection::new(LINE, 0);
    take_all(
        &mut projection,
        vec![heard(1, 20002), started(2, 0, &[1]), heard(3, 20003)],
    );
    projection.forked(LANE, 1, &[3]);
    projection
}

#[test]
fn a_fresh_lane_answers_whom_it_was_given_and_copies_what_came_after_the_fork() {
    let mut projection = forked();
    let busy = |user| Line::Busy {
        targets: vec![qq(user)],
    };
    assert_eq!(
        projection.lines(1),
        Lines {
            main: busy(20002),
            lanes: vec![busy(20003)],
            parallel: 1
        }
    );
    assert_eq!(projection.answering(true), ["m3"], "还没开轮：分派给它的");
    assert_eq!(projection.answering(false), ["m1"]);
    let carries = projection.to_copy(LANE, None);
    assert_eq!(carries.len(), 1, "第 1 条在分叉点以内，不抄");
    assert_eq!((carries[0].msg, carries[0].id.as_str()), (3, "c3/branch"));
    let params = &carries[0].params;
    assert_eq!(params["text"], "嗨");
    assert_eq!(
        params["as"],
        json!({"external": "qq:20003", "role": "member"})
    );
    assert_eq!(
        params["venue"],
        json!({"msg": "m3", "name": "某人", "ambient": true})
    );
    assert_eq!(params["attachments"][0]["name"], "msg-1-1");
    assert_eq!(projection.targets(LANE, &[1, 3]), [1], "没抄进去的不要");
    projection.copied(LANE, 3, 3);
    assert!(projection.to_copy(LANE, None).is_empty(), "抄过的不再抄");
    assert_eq!(projection.targets(LANE, &[1, 3]), [1, 3]);
}

#[test]
fn a_lane_speaks_in_main_numbers_and_its_turns_count() {
    let mut projection = forked();
    // 支线：第 2 条 `session.created`，第 3 条抄来的 B，第 4 条开一轮。
    projection.copied(LANE, 3, 3);
    let took = projection.lane_take(LANE, &started(4, 0, &[3]));
    assert_eq!(
        took,
        Seen::Took {
            turn: 4,
            msgs: vec![3]
        }
    );
    assert_eq!(projection.turns(&[]).len(), 2, "支线开的回合也算进限流");
    assert_eq!(projection.answering(true), ["m3"]);
    assert_eq!(
        projection.origin(LANE).map(|origin| origin.sender),
        Some(qq(20003))
    );
    assert_eq!(projection.turn(LANE), Some(4));
    let Seen::Spoke(speaking) = projection.lane_take(LANE, &assistant(5, 4)) else {
        panic!("她说了一段");
    };
    assert_eq!((speaking.line.as_str(), speaking.turn), (LANE, 4));
    assert_eq!(speaking.to, [qq(20003)]);
    assert_eq!(
        speaking.aim.map(|aim| aim.msg),
        Some(Some("m3".to_string()))
    );
    assert_eq!(
        projection.lane_take(LANE, &assistant(5, 4)),
        Seen::Nothing,
        "重的不收"
    );
    assert_eq!(
        projection.lane_take("别的", &started(6, 0, &[3])),
        Seen::Nothing
    );
    assert_eq!(projection.lane_take(LANE, &ended(6, 4)), Seen::Ended(4));
    assert_eq!(
        projection.lines(1).lanes,
        [Line::Idle],
        "完了、还没封存：照样占着"
    );
    assert!(projection.answering(true).is_empty());
    assert_eq!(projection.lane_take(LANE, &sealed(7)), Seen::Sealed);
    assert!(!projection.has_lane(LANE));
    assert!(projection.lines(1).lanes.is_empty());
}

#[test]
fn a_turn_the_lane_opens_for_what_joined_answers_whoever_joined() {
    let mut projection = forked();
    projection.copied(LANE, 3, 3);
    projection.lane_take(LANE, &started(4, 0, &[3]));
    take_all(&mut projection, vec![heard(5, 20004)]);
    projection.copied(LANE, 5, 5);
    // 支线第 5 条是抄来的、第 6 条把它并进第 4 轮；没再请求就完了，核心接着开第 8 轮。
    let took = projection.lane_take(LANE, &joined(6, 4, &[5]));
    assert_eq!(
        took,
        Seen::Took {
            turn: 4,
            msgs: vec![5]
        }
    );
    projection.lane_take(LANE, &ended(7, 4));
    let mut again = started(8, 0, &[]);
    if let miyu_client::events::Body::TurnStarted(started) = &mut again.body {
        started.trigger = miyu_client::ids::Seq::new(6);
    }
    projection.lane_take(LANE, &again);
    assert_eq!(
        projection.origin(LANE).map(|origin| origin.sender),
        Some(qq(20004))
    );
}

#[test]
fn turn_numbers_that_collide_stay_on_their_own_line() {
    let mut projection = forked();
    projection.copied(LANE, 3, 3);
    // 判过要回第 3 条；支线的第 2 轮收了它（主线的这一轮也是第 2 轮）。
    take_all(&mut projection, vec![decided(4, &[3], "reply")]);
    projection.lane_take(LANE, &started(2, 0, &[3]));
    projection.queued(LANE, 2, "支线的");
    take_all(&mut projection, vec![queued(5, LINE, 2, "主线的")]);
    let main = take_all(&mut projection, vec![assistant(6, 2)]);
    assert_eq!(main[0].sent, ["主线的"], "去重只看这条线");
    let Seen::Spoke(lane) = projection.lane_take(LANE, &assistant(3, 2)) else {
        panic!("她在支线说了一段");
    };
    assert_eq!(lane.sent, ["支线的"]);
    // 支线那一轮发出去的一段：她的最后一段照回执自己的序号，不照主线第 2 轮那一段的。
    take_all(
        &mut projection,
        vec![said_at(7, 0, 20005), delivered(8, LANE, 2)],
    );
    let main = take_all(&mut projection, vec![assistant(9, 2)]);
    assert!(main[0].last_is_own, "回执在第 7 条人话后面");
    // 主线第 2 轮完了：支线收的那一条还没回完。
    take_all(&mut projection, vec![ended(10, 2)]);
    assert_eq!(pendings(&projection).len(), 1);
    projection.lane_take(LANE, &ended(4, 2));
    assert!(pendings(&projection).is_empty(), "支线那一轮完了才算回完");
}

#[test]
fn messages_are_kept_only_while_a_line_may_still_need_them() {
    let mut projection = Projection::new(LINE, 0);
    take_all(&mut projection, vec![heard(1, 20002)]);
    projection.forked(LANE, 0, &[1]);
    assert!(
        projection.to_copy(LANE, None).is_empty(),
        "主线闲着、没有支线时来的：不留"
    );
    let mut projection = forked();
    take_all(&mut projection, vec![ended(4, 2), heard(5, 20005)]);
    projection.copied(LANE, 3, 3);
    assert_eq!(
        projection
            .to_copy(LANE, None)
            .iter()
            .map(|carry| carry.msg)
            .collect::<Vec<_>>(),
        [5],
        "主线闲了，支线还在：分叉点以后的照留"
    );
    projection.lane_take(LANE, &sealed(9));
    take_all(&mut projection, vec![heard(6, 20005)]);
    projection.forked("又一条", 0, &[6]);
    assert!(
        projection.to_copy("又一条", None).is_empty(),
        "都用不着了的丢了"
    );
    projection.dropped("又一条");
    assert!(!projection.has_lane("又一条"));
}
