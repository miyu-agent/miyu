//! 在路上的一次被丢下（施工 MD-2）：没等到回答的发 `driver.cancel`、不再收增量；答完了的什么都不发。

use tokio::sync::mpsc;

use super::*;

#[test]
fn a_dropped_call_is_cancelled_and_stops_receiving() {
    let (out, mut written) = mpsc::channel(8);
    let peer = Peer::new(out);
    for (done, cancelled) in [(false, true), (true, false)] {
        let (deltas, mut heard) = mpsc::unbounded_channel();
        let (asks, mut asked) = mpsc::unbounded_channel();
        peer.stream("d7", (deltas, asks));
        drop(Live {
            peer: peer.clone(),
            call: "d7".to_string(),
            done,
        });
        let line = written.try_recv().ok();
        assert_eq!(
            line.as_deref(),
            cancelled
                .then_some(r#"{"jsonrpc":"2.0","method":"driver.cancel","params":{"call":"d7"}}"#),
            "答完了没有：{done}"
        );
        peer.delta(json!({"call": "d7", "deltas": []}));
        assert!(heard.try_recv().is_err(), "丢下以后的增量不再收");
        assert!(
            peer.approve("d7", json!({})).is_none() && asked.try_recv().is_err(),
            "丢下以后要人确认的不再收（施工 ACP-8）"
        );
    }
}

/// 要人确认的（施工 ACP-8）：只交给这个连接上在进行的那一次；别的编号的不收。
#[test]
fn only_the_call_in_progress_on_this_connection_can_ask() {
    let (out, _written) = mpsc::channel(8);
    let peer = Peer::new(out);
    let (deltas, _heard) = mpsc::unbounded_channel();
    let (asks, mut asked) = mpsc::unbounded_channel();
    peer.stream("d7", (deltas, asks));
    assert!(
        peer.approve("d8", json!({"call": "d8"})).is_none(),
        "别的编号"
    );
    assert!(asked.try_recv().is_err());
    let answered = peer.approve("d7", json!({"call": "d7", "index": 0}));
    assert!(answered.is_some());
    let ask = asked.try_recv().expect("交过去了");
    assert_eq!(ask.params, json!({"call": "d7", "index": 0}), "参数原样");
    let other = Peer::new(mpsc::channel(8).0);
    assert!(
        other.approve("d7", json!({})).is_none(),
        "别的连接上的同一个编号"
    );
}
