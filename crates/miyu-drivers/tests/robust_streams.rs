//! 鲁棒性（2026-10-22）：供应商、中转站一直往下发、就是不收尾的流——一行一直不换行，或者一条事件的 `data:` 一行接一行、
//! 一直没有空行——解码器的缓冲（`sse.rs` 的 `line`、`data`）没有上限，一直攒到核心的内存用完；读流那一头只有空闲超时
//! （`miyu-http` 的 `send.rs`），不停地发就永远不超时。
//!
//! 期望：攒到一个上限（这里按 64 MiB 算，具体多少由修的人定）就不再攒，解码器当这一次出错收尾（`done()` 为真，`finish()`
//! 带错）。修之前这条是红的。

use miyu_drivers::{anthropic, openai_chat, openai_responses};

/// 喂多少还没停就算没上限。
const MOST: usize = 128 * 1024 * 1024;
/// 一片多大。
const PIECE: usize = 1024 * 1024;

/// 三家的解码器，统一成喂、问停没停、收尾。
trait Fed {
    fn feed_bytes(&mut self, bytes: &[u8]);
    fn stopped(&self) -> bool;
    fn errored(self: Box<Self>) -> bool;
}

macro_rules! fed {
    ($decoder:ty) => {
        impl Fed for $decoder {
            fn feed_bytes(&mut self, bytes: &[u8]) {
                drop(self.feed(bytes));
            }
            fn stopped(&self) -> bool {
                self.done()
            }
            fn errored(self: Box<Self>) -> bool {
                self.finish().error.is_some()
            }
        }
    };
}

fed!(openai_chat::Decoder);
fed!(anthropic::Decoder);
fed!(openai_responses::Decoder);

fn decoders() -> Vec<(&'static str, Box<dyn Fed>)> {
    vec![
        ("openai_chat", Box::new(openai_chat::Decoder::new())),
        ("anthropic", Box::new(anthropic::Decoder::new())),
        (
            "openai_responses",
            Box::new(openai_responses::Decoder::new()),
        ),
    ]
}

/// 照 `piece` 一片片喂，最多 [`MOST`]：交回停没停、停的时候带没带错、喂了多少。
fn endless(mut decoder: Box<dyn Fed>, head: &[u8], piece: &[u8]) -> (bool, bool, usize) {
    decoder.feed_bytes(head);
    let mut fed = head.len();
    while fed < MOST {
        decoder.feed_bytes(piece);
        fed += piece.len();
        if decoder.stopped() {
            return (true, decoder.errored(), fed);
        }
    }
    (false, false, fed)
}

#[test]
fn a_line_that_never_ends_is_cut_off() {
    let piece = vec![b'a'; PIECE];
    let mut problems = Vec::new();
    for (name, decoder) in decoders() {
        let (stopped, errored, fed) = endless(decoder, b"data: ", &piece);
        if !(stopped && errored) {
            problems.push(format!(
                "{name}：喂了 {} MiB 不换行的一行，停了 {stopped}，带错 {errored}",
                fed >> 20
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn an_event_that_never_ends_is_cut_off() {
    let mut piece = b"data: ".to_vec();
    piece.extend(std::iter::repeat_n(b'a', PIECE - 7));
    piece.push(b'\n');
    let mut problems = Vec::new();
    for (name, decoder) in decoders() {
        let (stopped, errored, fed) = endless(decoder, b"", &piece);
        if !(stopped && errored) {
            problems.push(format!(
                "{name}：喂了 {} MiB 没有空行的一条事件，停了 {stopped}，带错 {errored}",
                fed >> 20
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
