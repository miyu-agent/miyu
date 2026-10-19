//! 叫停人叫的整理（施工 R-22，`docs/blueprint/memory.md` 第七条第 13 款）：会话的 `/stop` 连这个会话正在跑的 `/dream` 一起停。
//! 合完了的批留着，正在合的那一批放弃、下次整理再合，不回退。定时的、协议带 `persona` 的那几趟听不见。

use tokio::sync::watch;

/// 一个会话叫停整理的那一头（放在它的 `Extractor` 里）：叫一次往上数一，这时正在听的都听得见，之后才叫的整理不受影响。
#[derive(Debug)]
pub(crate) struct Hushes(watch::Sender<u64>);

impl Default for Hushes {
    fn default() -> Hushes {
        Hushes(watch::Sender::new(0))
    }
}

impl Hushes {
    /// 叫停这时正在整理的，交回有没有（还拿着听的那一头的：整理完了就放下）。
    pub(crate) fn hush(&self) -> bool {
        self.0.send_modify(|count| *count += 1);
        self.0.receiver_count() > 0
    }

    /// 一次整理从这时起听着。
    pub(crate) fn listen(&self) -> Hush {
        Hush(Some(self.0.subscribe()))
    }
}

/// 一次整理听的那一头；默认的听不见。
#[derive(Debug, Clone, Default)]
pub(crate) struct Hush(Option<watch::Receiver<u64>>);

impl Hush {
    /// 叫停过了。
    pub(crate) fn heard(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|listening| listening.has_changed().unwrap_or(false))
    }

    /// 等到叫停；听不见的（默认的、会话没了的）一直等。
    pub(crate) async fn wait(&self) {
        if let Some(listening) = &self.0 {
            let mut listening = listening.clone();
            if listening.changed().await.is_ok() {
                return;
            }
        }
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests {
    use super::{Hush, Hushes};

    #[tokio::test]
    async fn only_dreams_listening_when_hushed_hear_it() {
        let hushes = Hushes::default();
        assert!(!hushes.hush(), "没有在听的");
        let before = hushes.listen();
        assert!(!before.heard());
        assert!(hushes.hush(), "有一个在听");
        assert!(before.heard());
        before.wait().await;
        let after = hushes.listen();
        assert!(!after.heard());
        assert!(!Hush::default().heard());
        drop((before, after));
        assert!(!hushes.hush(), "都放下了");
    }
}
