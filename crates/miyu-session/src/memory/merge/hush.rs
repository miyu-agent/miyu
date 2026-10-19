//! 叫停人叫的整理（施工 R-22，`docs/blueprint/memory.md` 第七条第 13 款）：会话的 `/stop`、`/dream stop` 停这个会话叫的那一趟，
//! 协议的 `memory.dream_stop` 停这一间人叫的那一趟，不管是哪一路叫的（施工 R-23）。合完了的批留着，正在合的那一批放弃、下次
//! 整理再合，不回退。定时的听不见。

use tokio::sync::watch;

use crate::memory::Keeper;

/// 叫停整理的一头：一个会话一份（放在它的 `Extractor` 里），一间一份（`Merges` 里，施工 R-23）。叫一次往上数一，这时正在听的都
/// 听得见，之后才叫的整理不受影响。
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
        Hush {
            session: Some(self.0.subscribe()),
            room: None,
        }
    }
}

/// 一次整理听的那一头：叫它的会话的、这一间的（施工 R-23），哪一头叫停都停；默认的听不见。
#[derive(Debug, Clone, Default)]
pub(crate) struct Hush {
    session: Option<watch::Receiver<u64>>,
    room: Option<watch::Receiver<u64>>,
}

impl Hush {
    /// 再听这一间的那一头（施工 R-23）。
    pub(crate) fn and_room(self, room: &Hushes) -> Hush {
        Hush {
            room: Some(room.0.subscribe()),
            ..self
        }
    }

    /// 叫停过了。
    pub(crate) fn heard(&self) -> bool {
        [&self.session, &self.room]
            .into_iter()
            .flatten()
            .any(|listening| listening.has_changed().unwrap_or(false))
    }

    /// 等到叫停；听不见的（默认的、叫停的那一头没了的）一直等。
    pub(crate) async fn wait(&self) {
        tokio::select! {
            () = until(self.session.as_ref()) => {}
            () = until(self.room.as_ref()) => {}
        }
    }
}

/// 等这一头叫停；没有的、那一头没了的一直等。
async fn until(listening: Option<&watch::Receiver<u64>>) {
    if let Some(listening) = listening {
        let mut listening = listening.clone();
        if listening.changed().await.is_ok() {
            return;
        }
    }
    std::future::pending::<()>().await;
}

impl Keeper {
    /// 人叫的整理听的那一头：`hush`（叫它的会话的）再加这一间的（施工 R-23）。
    pub(crate) fn hush_for(&self, hush: Hush) -> Hush {
        self.memory.merges.listen(self.room(), hush)
    }

    /// 叫停这一间这时人叫的整理（施工 R-23，`memory.dream_stop`，`memory.md` 第七条第 13 款）：不管是哪一路叫的，定时的
    /// 不停；交回有没有在跑的。合完了的批留着，正在合的那一批放弃、下次再合。
    pub fn stop_dream(&self) -> bool {
        self.memory.merges.hush(self.room())
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

    #[tokio::test]
    async fn either_end_stops_it() {
        let (session, room) = (Hushes::default(), Hushes::default());
        let both = session.listen().and_room(&room);
        assert!(!both.heard());
        room.hush();
        assert!(both.heard(), "这一间叫停的也听得见");
        both.wait().await;
        let both = session.listen().and_room(&room);
        session.hush();
        assert!(both.heard(), "会话叫停的也听得见");
        both.wait().await;
    }
}
