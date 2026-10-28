//! 抽取只读上次读到以后那一截日志（施工 E-5 补，`docs/blueprint/memory.md` 第六条第 2 款）：头一次整份读、接上以后，更早的
//! 那一截读不出来了也照样抽，人叫的整理先抽的那一次也一样——两路都只从记下的那一处读起，不再每次整份读一遍。

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::session::Command;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat};
use crate::support::*;

/// 抽取的请求（合并、写关键词的不算）。
fn extractions(server: &Server) -> Vec<String> {
    (0..server.received().len())
        .map(|at| asked(server, at))
        .filter(|text| text.starts_with("Find what is worth remembering"))
        .collect()
}

/// 等到抽取的请求有 `count` 个（最多 10 秒）。
async fn extracted_times(server: &Server, count: usize) -> Vec<String> {
    for _ in 0..200 {
        let found = extractions(server);
        if found.len() >= count {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "十秒没等到第 {count} 次抽取，有 {}",
        extractions(server).len()
    );
}

/// 弄坏日志里带着 `words` 的那一行：长短不变，读不懂了。整份读从此读不出来，从后面一处读起的照样读得出。
fn break_line(home: &Home, session: &SessionId, words: &str) {
    let path = home.root.session_dir(session).join("000000000001.jsonl");
    let text = std::fs::read_to_string(&path).expect("读得到日志");
    let at = text.find(words).expect("说过这句");
    let start = text[..at].rfind('\n').map_or(0, |at| at + 1);
    let mut bytes = text.into_bytes();
    bytes[start..start + 6].copy_from_slice(b"######");
    std::fs::write(&path, bytes).expect("写得进");
}

/// 说一句、等这一轮结束：只看推来的，不读日志（更早的一截已经弄坏了）。
async fn say_on(handle: &Handle, n: usize, words: &str) {
    let mut pushes = watch(handle).await;
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: None,
    };
    handle
        .command(id(&format!("cmd-{n}")), alice(), said)
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 等这个会话记下的抽到了哪变了（上一次抽取走完了），交回新的；最多 10 秒。抽取记下抽到了哪以后还要写关键词、补向量才算
/// 走完，人叫的整理碰上还在走的那一次不抽：再等一小会儿。
async fn extracted_past(home: &Home, session: &SessionId, before: Option<Seq>) -> Option<Seq> {
    for _ in 0..200 {
        let now = mark(home, session);
        if now != before {
            tokio::time::sleep(Duration::from_millis(300)).await;
            return now;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("十秒没等到抽取记下抽到了哪");
}

#[tokio::test]
async fn once_caught_up_extraction_reads_only_after_the_last_mark() {
    let server = Server::start(vec![
        every_turn("用户在聊天"),
        every_turn("用户还在聊天"),
        every_turn("用户又说了一句"),
        stream(&serde_json::json!({"summary": "用户在聊天。"}).to_string()),
        every_turn("用户接着聊"),
    ])
    .await;
    let mut home = Home::new();
    organizer_busy(&mut home, &server, "extract_max_turns = 3\n", None);
    let script = Script::new((0..10).map(|_| Play::Says("好。")));
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    for (n, words) in ["第一句", "第二句", "第三句"].into_iter().enumerate() {
        chat(&home, &handle, n + 1, words).await;
    }
    // 头一次还接不上，整份读、顺手接上；第二次上次抽到的那一条以前还没记下哪一处，也整份读。
    extracted_times(&server, 1).await;
    let first = extracted_past(&home, handle.id(), None).await;
    for (n, words) in ["第四句", "第五句", "第六句"].into_iter().enumerate() {
        chat(&home, &handle, n + 4, words).await;
    }
    let second = extracted_times(&server, 2).await;
    assert!(second[1].contains("第四句"), "{}", second[1]);
    let second_mark = extracted_past(&home, handle.id(), first).await;
    break_line(&home, handle.id(), "第一句");
    // 人叫的整理先抽的那一次：上一次已经走完，从记下的那一处读起。
    say_on(&handle, 7, "第七句").await;
    handle.dream().await.expect("会话在跑").expect("整理了");
    let third = extracted_times(&server, 3).await;
    assert!(
        third[2].contains("第七句") && !third[2].contains("第六句"),
        "人叫的整理先抽的那一次也从记下的那一处读起：{}",
        third[2]
    );
    extracted_past(&home, handle.id(), second_mark).await;
    // 不等闲的那一次也是。
    say_on(&handle, 8, "第八句").await;
    say_on(&handle, 9, "第九句").await;
    say_on(&handle, 10, "第十句").await;
    let fourth = extracted_times(&server, 4).await;
    assert!(
        fourth[3].contains("第八句") && fourth[3].contains("第十句"),
        "更早的一截读不出来也照样抽：{}",
        fourth[3]
    );
    stop(&handle).await;
}
