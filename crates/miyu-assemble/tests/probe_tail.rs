//! 请求形状探针：respond 带的那一句（施工 RT-1，`Request::tail`，core1 11-07 定）。真内核、真组装器照剧本跑：旁听一句、照它开一轮
//! 带那一句，调一次工具再答（工具第二步），接着人说一句开第二轮。三家驱动各编一遍：
//!
//! - 去掉那一句，五条性质照旧成立，和同一份剧本不带它跑的逐字节一样（前面零变化）；
//! - 每次请求带和不带只差插进去的那一段，就是那一句；
//! - 下一次请求（工具第二步、第二轮）把上一次那一句前面的字节一个不少地接着用：缓存命中只差那一句；anthropic 上一次最后那个
//!   打点（那一句前面那一块）下一次在同一处照样打了。

use crate::support::{Sent, anthropic_wire, check, responses_wire, sent, stage, wire};
use miyu_drivers::anthropic;
use miyu_kernel::request::Request;
use miyu_kernel::testkit::{Line, Play, Stage};

const TAIL: &str = "（群里的提示）";

const MARK: &str = ",\"cache_control\":{\"type\":\"ephemeral\"}";

fn script(tail: Option<&str>) -> Stage {
    let mut s = stage();
    s.model([
        Line::calls("我看一下。", &[("read", r#"{"path":"a.rs"}"#)]),
        Line::says("看完了。"),
        Line::says("嗯。"),
    ]);
    s.tools([Play::done("fn main() {}")]);
    let heard = s.overhear("帮我看看 a.rs");
    s.respond(&[heard], tail);
    s.say("还有吗");
    s
}

fn stripped(request: &Request) -> Request {
    let mut request = request.clone();
    request.tail = None;
    request
}

/// 三家编出来的字：openai-chat、去掉打点的 anthropic、responses。
fn encoded(request: &Request) -> [(&'static str, String); 3] {
    let utf8 = |bytes: Vec<u8>| String::from_utf8(bytes).expect("UTF-8");
    [
        ("openai-chat", utf8(wire(request).body)),
        (
            "anthropic",
            utf8(anthropic::unmarked(&anthropic_wire(request)).body),
        ),
        ("responses", utf8(responses_wire(request).body)),
    ]
}

fn shared(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

#[test]
fn without_the_tail_the_properties_hold_and_the_bytes_are_the_same() {
    let with = script(Some(TAIL));
    let mut plain: Vec<Sent> = sent(&with);
    for one in &mut plain {
        one.request = stripped(&one.request);
    }
    if let Err(why) = check(&plain) {
        panic!("{why}");
    }
    let without = script(None);
    assert_eq!(with.requests().len(), 3);
    for ((_, a), (_, b)) in with.requests().iter().zip(without.requests()) {
        assert_eq!(
            stripped(a).canonical_bytes(),
            b.canonical_bytes(),
            "前面零变化"
        );
        for ((name, x), (_, y)) in encoded(&stripped(a)).iter().zip(encoded(b)) {
            assert_eq!(*x, y, "{name}");
        }
    }
    let tails: Vec<Option<&str>> = with
        .requests()
        .iter()
        .map(|(_, request)| request.tail.as_deref())
        .collect();
    assert_eq!(
        tails,
        [Some(TAIL), Some(TAIL), None],
        "这一轮两步带，第二轮不带"
    );
}

#[test]
fn each_request_only_gains_the_tail_inserted_once() {
    let s = script(Some(TAIL));
    for (index, (_, request)) in s.requests().iter().enumerate() {
        if request.tail.is_none() {
            continue;
        }
        for ((name, with), (_, without)) in encoded(request).iter().zip(encoded(&stripped(request)))
        {
            let at = shared(with, &without);
            let added = with.len() - without.len();
            assert!(
                with[at..at + added].contains(TAIL),
                "第 {} 次 {name}：插进去的是那一句",
                index + 1
            );
            assert_eq!(
                &with[at + added..],
                &without[at..],
                "第 {} 次 {name}：后面原样",
                index + 1
            );
        }
    }
}

#[test]
fn the_next_request_keeps_every_byte_before_the_tail() {
    let s = script(Some(TAIL));
    let requests = s.requests();
    for pair in requests.windows(2) {
        let (before, now) = (&pair[0].1, &pair[1].1);
        for ((name, then), (_, next)) in encoded(before).iter().zip(encoded(now)) {
            let tail_at = then.find(TAIL).expect("上一次带着那一句");
            // 接在那一句前面的分隔（`\n`、anthropic 那一块的开头）跟着它走：最多差这么几个字节。
            let joint = ",{\"type\":\"text\",\"text\":\"".len();
            let kept = shared(then, &next);
            assert!(
                kept + joint >= tail_at,
                "{name}：下一次只丢那一句（共 {kept}，那一句从 {tail_at} 起）"
            );
        }
        // anthropic 的打点：上一次最后那个打在那一句前面，下一次在同一处照样打了。
        let marked = |request: &Request| String::from_utf8(anthropic_wire(request).body).unwrap();
        let (then, next) = (marked(before), marked(now));
        let last = then.rfind(MARK).expect("上一次打了点");
        assert!(last < then.find(TAIL).unwrap(), "那一句后面不打点");
        let plain = |body: &str| body.replace(MARK, "");
        let upto = plain(&then[..last]);
        assert!(
            next.match_indices(MARK)
                .any(|(at, _)| plain(&next[..at]) == upto),
            "下一次在上一次最后那个打点的同一处也打了"
        );
    }
}
