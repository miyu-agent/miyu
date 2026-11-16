//! 叫停一次请求（施工 MC-1 从 `port.rs` 挪出来，那边放不下了）。

use tokio::sync::oneshot;

/// 叫停一次请求：会话不要这次请求了，或者会话停了（actor 放下了叫停的那一头）。会话停了就没人要
/// 结果了，接着读只是白花 token。说完了以后放下的，请求已经不在读了，停不停都一样。
#[derive(Debug)]
pub struct Cancel(oneshot::Receiver<()>);

impl Cancel {
    pub(crate) fn new(receiver: oneshot::Receiver<()>) -> Cancel {
        Cancel(receiver)
    }

    /// 等到被叫停。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "明着叫停和放下了一个意思：都是叫停"
    )]
    pub async fn wait(self) {
        let _ = self.0.await;
    }
}
