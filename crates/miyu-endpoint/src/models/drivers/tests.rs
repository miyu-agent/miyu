//! 在路上的一次被丢下（施工 MD-2）：没等到回答的发 `driver.cancel`、不再收增量；答完了的什么都不发。

use tokio::sync::mpsc;

use super::*;

#[test]
fn a_dropped_call_is_cancelled_and_stops_receiving() {
    let (out, mut written) = mpsc::channel(8);
    let peer = Peer::new(out);
    for (done, cancelled) in [(false, true), (true, false)] {
        let (deltas, mut heard) = mpsc::unbounded_channel();
        peer.stream("d7", deltas);
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
    }
}
