//! 一次转述的结果送回哪里（施工 8-17；施工 ACP-8 从 `port.rs` 挪出来，那个文件到了行数上限）。

use tokio::sync::mpsc;

use miyu_kernel::id::ContentHash;
use miyu_kernel::origin::Model;

use super::Back;

/// 一次转述的结果送回哪里（施工 8-17）：成了的交替它看的模型和转述，没成的交为什么。都送回 actor 的收件箱。
#[derive(Debug)]
pub struct Sight {
    blob: ContentHash,
    back: mpsc::UnboundedSender<Back>,
}

impl Sight {
    pub(crate) fn new(blob: ContentHash, back: mpsc::UnboundedSender<Back>) -> Sight {
        Sight { blob, back }
    }

    /// 转述成了：替它看的是 `model`，转述是 `text`（去掉了前后空白，不是空的）。
    pub fn seen(self, model: Model, text: String) {
        self.send(Ok((model, text)));
    }

    /// 转述没成：为什么，记进运行日志。
    pub fn unseen(self, why: String) {
        self.send(Err(why));
    }

    #[expect(
        clippy::let_underscore_must_use,
        reason = "会话停了就送不进去：转述没人要了，丢掉"
    )]
    fn send(self, seen: Result<(Model, String), String>) {
        let _ = self.back.send(Back::Described {
            blob: self.blob,
            seen,
        });
    }
}
