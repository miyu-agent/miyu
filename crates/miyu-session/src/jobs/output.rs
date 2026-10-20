//! 一条后台命令的输出文件（`docs/blueprint/store.md` 的 `jobs/<编号>.out`）：读的线程一段段写进去，不截；报了结束就关上，
//! 之后读到的不再写（还拿着管道的孙进程）。结束时整份存成 blob，数出有多少个字。每写进一段、关上时叫一声（施工 V-6 上，
//! 会话的任务那一层的信号）：头带 `wait` 读 `job.output` 的等着它。

use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::watch;

use miyu_kernel::id::ContentHash;
use miyu_store::blob::Blobs;

use crate::TARGET;

/// 一条后台命令的输出文件。
pub(super) struct Output {
    path: PathBuf,
    /// 写的一头；关上了、写不进去了的是空的。
    file: Mutex<Option<File>>,
    /// 写进一段、关上时叫一声（施工 V-6 上）。
    changes: Arc<watch::Sender<u64>>,
    /// 写进去了多少字节（施工 7-12 下）：过了上限就停掉这条命令。
    written: AtomicU64,
}

impl Output {
    /// 在 `path` 上开好的 `file`；写进一段、关上时叫 `changes` 一声。
    pub(super) fn new(path: PathBuf, file: File, changes: Arc<watch::Sender<u64>>) -> Output {
        Output {
            path,
            file: Mutex::new(Some(file)),
            changes,
            written: AtomicU64::new(0),
        }
    }

    /// 写一段。关上了的不写；写不进去的（磁盘满了之类）记一行，关上，之后的都不写：命令照跑，读的线程照读，不让它
    /// 卡在写满的管道上。
    pub(super) fn write(&self, text: &str) {
        let mut file = self.lock();
        let Some(open) = file.as_mut() else {
            return;
        };
        match open.write_all(text.as_bytes()) {
            Ok(()) => {
                self.written.fetch_add(text.len() as u64, Ordering::Relaxed);
            }
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "job output not written");
                *file = None;
            }
        }
        drop(file);
        bump(&self.changes);
    }

    /// 写进去了多少字节。
    pub(super) fn written(&self) -> u64 {
        self.written.load(Ordering::Relaxed)
    }

    /// 关上：之后读到的不再写。
    pub(super) fn close(&self) {
        *self.lock() = None;
        bump(&self.changes);
    }

    /// 整份存成 blob，交回它的哈希、有多少个字（Unicode 字符）、最后一段（施工 7-13，[`TAIL`]）。读不出来、存不下来的
    /// 记一行，三样都没有：回报里不写输出，她也就不会去读一份不在的。碰磁盘，在阻塞线程里调。
    pub(super) fn stored(&self, blobs: &Blobs) -> Stored {
        // 边读边存（施工 7-12 下）：输出最多 5 GB，不整份读进内存；最后一段只留末尾够用的字节。
        let mut chars = 0;
        let mut end: VecDeque<u8> = VecDeque::new();
        let stored = blobs.put_file(&self.path, |bytes| {
            chars += count(bytes);
            end.extend(bytes);
            let over = end.len().saturating_sub(TAIL_BYTES);
            end.drain(..over);
        });
        match stored {
            Ok(hash) => Stored {
                output: Some(hash),
                chars: Some(chars),
                tail: (chars > 0).then(|| tail_of(end.make_contiguous(), TAIL)),
            },
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "job output not stored");
                Stored::default()
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<File>> {
        self.file.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 叫一声：计数加一，等着的醒来再看一眼。
pub(super) fn bump(changes: &watch::Sender<u64>) {
    changes.send_modify(|count| *count = count.wrapping_add(1));
}

/// 回报里带的最后一段最多几个字（施工 7-13，2026-10-20 项目主人提的：输出不长就直接带上）。
pub(crate) const TAIL: usize = 2000;

/// 留末尾多少字节就够 [`TAIL`] 个字：一个字最多 4 个字节，再多留一点好切在行首。
const TAIL_BYTES: usize = TAIL * 4 + 4;

/// 存好的输出：blob、字数、最后一段。
#[derive(Debug, Default)]
pub(super) struct Stored {
    pub(super) output: Option<ContentHash>,
    pub(super) chars: Option<u64>,
    pub(super) tail: Option<String>,
}

/// 末尾的 `end` 字节里最后 `most` 个字：开头切在一个字中间的照 `\u{FFFD}` 解，再去掉；不是从一行开头起的，从下一行起
/// （下一行在后一半里才这样切，不然照字切）。
pub(crate) fn tail_of(end: &[u8], most: usize) -> String {
    let text = String::from_utf8_lossy(end);
    let total = text.chars().count();
    if total <= most {
        return text.into_owned();
    }
    let start = text
        .char_indices()
        .nth(total - most)
        .map_or(text.len(), |(at, _)| at);
    let part = &text[start..];
    if text[..start].ends_with('\n') {
        return part.to_string();
    }
    match part.find('\n') {
        Some(at) if at < part.len() / 2 => part[at + 1..].to_string(),
        _ => part.to_string(),
    }
}

/// 有多少个字：写进去的都是解好的 UTF-8，不是接续字节的每个字节算一个字，和前台的数法一样。
fn count(bytes: &[u8]) -> u64 {
    bytes.iter().filter(|&&byte| byte & 0xC0 != 0x80).count() as u64
}
