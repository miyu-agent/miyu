//! 中途订阅补推正在流的块（施工 9-8 六补，`protocol.md`「补发」第 4 条）：一轮流到一半时订阅的，先收到这次请求到目前为止
//! 的块（`start`、拼好的字），之后的增量照常；没有在流的请求的什么都不补。

use std::sync::Arc;

use miyu_kernel::accumulate::Kind;
use miyu_kernel::event::{ModelDelta, Piece, TransientBody};
use miyu_kernel::session::{Command, Queued};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Pushed, Subscription};

use crate::support::{Home, ask, say, until_turn_ends, watch, within};

/// 读到第 `n` 段模型增量为止。
async fn deltas(subscription: &mut Subscription, n: usize) {
    within("增量", async {
        let mut seen = 0;
        while seen < n {
            let next = subscription.next().await.expect("订阅没断");
            if matches!(&*next, Pushed::Transient(t) if matches!(t.body, TransientBody::ModelDelta(_))) {
                seen += 1;
            }
        }
    })
    .await;
}

/// 已经到了的推送里的模型增量：第几块、哪一段。
fn caught(subscription: &mut Subscription) -> Vec<(usize, Piece)> {
    let mut out = Vec::new();
    while let Some(next) = subscription.try_next() {
        let next: Arc<Pushed> = next.expect("订阅没断");
        if let Pushed::Transient(one) = &*next
            && let TransientBody::ModelDelta(ModelDelta { index, piece, .. }) = &one.body
        {
            out.push((*index, piece.clone()));
        }
    }
    out
}

#[tokio::test]
async fn a_late_subscriber_gets_the_blocks_streamed_so_far() {
    let home = Home::new();
    let script = Script::new([Play::Holds]);
    let handle = home.create(&script).await;
    let mut first = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    // 开了个头的那两段（开始、一段字）都推过了，才中途订阅。
    deltas(&mut first, 2).await;
    let mut late = watch(&handle).await;
    assert_eq!(
        caught(&mut late),
        [
            (0, Piece::Start(Kind::Text)),
            (0, Piece::Text("…".to_string()))
        ],
        "先补到目前为止的块"
    );
    assert!(caught(&mut first).is_empty(), "早订阅的不重推");
    // 等着读的（`next`）也先拿到补的那两段。
    let mut waiting = watch(&handle).await;
    let mut read = Vec::new();
    for _ in 0..2 {
        let next = within("补的", waiting.next()).await.expect("订阅没断");
        if let Pushed::Transient(one) = &*next
            && let TransientBody::ModelDelta(ModelDelta { index, piece, .. }) = &one.body
        {
            read.push((*index, piece.clone()));
        }
    }
    assert_eq!(
        read,
        [
            (0, Piece::Start(Kind::Text)),
            (0, Piece::Text("…".to_string()))
        ]
    );

    ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut first).await;
    let mut after = watch(&handle).await;
    assert!(caught(&mut after).is_empty(), "没有在流的请求，什么都不补");
}
