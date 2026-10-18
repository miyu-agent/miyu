//! 一卷屏幕（从 `mod.rs` 挪出来：那一份到了 500 行，施工 G-3 上）。

use std::sync::{Arc, Mutex};

/// 一卷屏幕：标准输出、标准错误照先后记在一起。只看各自的缓冲区看不出两条通道交错的先后。
#[derive(Clone, Default)]
pub struct Tape(Arc<Mutex<Vec<(bool, u8)>>>);

/// 往屏幕上写的一支笔：`err` 的写标准错误。
pub struct Pen {
    tape: Tape,
    err: bool,
}

impl std::io::Write for Pen {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut tape = self.tape.0.lock().expect("没 panic");
        tape.extend(buf.iter().map(|byte| (self.err, *byte)));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Tape {
    /// 一支笔。
    pub fn pen(&self, err: bool) -> Pen {
        Pen {
            tape: self.clone(),
            err,
        }
    }

    /// 照 `pick` 挑出来的字节，写成字。
    pub fn text(&self, pick: impl Fn(bool) -> bool) -> String {
        let tape = self.0.lock().expect("没 panic");
        let bytes: Vec<u8> = tape
            .iter()
            .filter(|(err, _)| pick(*err))
            .map(|(_, byte)| *byte)
            .collect();
        String::from_utf8(bytes).expect("UTF-8")
    }
}
