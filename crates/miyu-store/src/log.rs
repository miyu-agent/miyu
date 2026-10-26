//! 会话日志（`docs/designs/07-存储.md` 第三节「事件日志」、第四节「写入与崩溃」）：一个会话一个
//! 目录，按段存成 JSONL；一批事件一次写入、一次同步，同步完了才算落盘；打开时自检（`log/open.rs`）。
//!
//! 一个会话只有一个写者，就是它的 actor，所以不加锁。内核的「追加事件」动作落到这里，写完了
//! 执行器送一条「落盘了」回去（`02-内核.md` 第四节「执行器怎么回动作」）。

mod open;

pub use open::{Flow, OpenError, first_event, read_events, read_marked, read_segments};

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use crate::durable::{create_dir, sync_dir};

/// 一段的上限，初值，实测再定（07 第三节）：写一批之前这一段已经到了它，就开下一段。
pub const SEGMENT_LIMIT: u64 = 64 * 1024 * 1024;

/// 日志里的一个位置（施工 3-8 七补）：哪一段、这一段照到第几个字节、下一条该是几号。会话列表的索引记着每一行照到日志
/// 的哪里（[`crate::index`]），列会话时只读它后面多出来的那一截（[`read_marked`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    /// 哪一段：这一段第一条的序号，也就是段的名字。
    pub segment: u64,
    /// 这一段照到第几个字节：一整行的末尾，不含后面没写完的半行。
    pub bytes: u64,
    /// 下一条该是几号。
    pub next: Seq,
}

/// 一个会话的日志，开着的，只往后追加。
#[derive(Debug)]
pub struct SessionLog {
    /// 会话的目录。
    dir: PathBuf,
    /// 正在写的那一段。
    file: File,
    /// 正在写的那一段叫什么：它第一条的序号（施工 3-8 七补，[`SessionLog::mark`] 要）。
    segment: u64,
    /// 这一段已经写了多少字节。
    size: u64,
    /// 下一条该是几号。
    next: Seq,
    /// 一段的上限。
    limit: u64,
}

impl SessionLog {
    /// 新会话：建好目录，和空的第一段。`limit` 是一段的上限，平时用 [`SEGMENT_LIMIT`]。
    ///
    /// # Errors
    ///
    /// 建不了目录；第一段已经有了（不覆盖）。
    pub fn create(dir: &Path, limit: u64) -> io::Result<SessionLog> {
        SessionLog::create_at(dir, limit, Seq::FIRST)
    }

    /// 同 [`SessionLog::create`]，第一条是 `first` 号（施工 C-8 上：分叉出来的会话接着分叉点往下数）：第一段照它起名。
    ///
    /// # Errors
    ///
    /// 同 [`SessionLog::create`]。
    pub fn create_at(dir: &Path, limit: u64, first: Seq) -> io::Result<SessionLog> {
        create_dir(dir)?;
        let file = new_segment(dir, first)?;
        Ok(SessionLog {
            dir: dir.to_path_buf(),
            file,
            segment: first.get(),
            size: 0,
            next: first,
            limit,
        })
    }

    /// 会话的目录：只读地读回整份日志时用（[`read_events`]，施工 4-7 上）。
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 下一条该是几号。
    pub fn next_seq(&self) -> Seq {
        self.next
    }

    /// 写到哪了（施工 3-8 七补）：正在写的那一段、它的长度、下一条该是几号。会话列表的索引照它记一行照到哪里。
    pub fn mark(&self) -> Mark {
        Mark {
            segment: self.segment,
            bytes: self.size,
            next: self.next,
        }
    }

    /// 追加一批，再同步：[`SessionLog::write`] 加 [`SessionLog::sync`]。返回时这一批都落了盘。
    ///
    /// # Errors
    ///
    /// 序号接不上（调用的一方的 bug，不写进去）；写不进、同步不了。
    pub fn append(&mut self, events: &[Event]) -> io::Result<()> {
        self.write(events)?;
        self.sync()
    }

    /// 同步正在写的那一段（`sync_data`：数据和读得出数据要的文件长度，施工 V-18）。之前 [`SessionLog::write`] 的
    /// 都落了盘：换段时前一段在那一刻已经同步过，新的一段只许新建地建、建好同步了目录。
    ///
    /// # Errors
    ///
    /// 同步不了。
    pub fn sync(&mut self) -> io::Result<()> {
        self.file.sync_data()
    }

    /// 追加一批，不同步（施工 V-18，`store.md` 第 5 条第 4 款）：拼成一块，一次写入。返回时这一批交给了系统：
    /// 程序崩了照样在，断电要等下一次 [`SessionLog::sync`] 才保得住。写之前这一段已经到了上限，先同步这一段、
    /// 再开下一段；一批不拆到两段里。
    ///
    /// # Errors
    ///
    /// 序号接不上（调用的一方的 bug，不写进去）；写不进；换段时同步不了。
    pub fn write(&mut self, events: &[Event]) -> io::Result<()> {
        let Some(first) = events.first() else {
            return Ok(());
        };
        let mut expected = self.next;
        let mut bytes = Vec::new();
        for event in events {
            if event.seq != expected {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "the next event in the log should be {expected}, got {}",
                        event.seq
                    ),
                ));
            }
            bytes.extend_from_slice(event.to_line().as_bytes());
            bytes.push(b'\n');
            expected = expected.next();
        }
        if self.size > 0 && self.size >= self.limit {
            // 前一段不再写了：先落盘，断电时不会有后一段、前一段却缺了一截。
            self.file.sync_data()?;
            self.file = new_segment(&self.dir, first.seq)?;
            self.segment = first.seq.get();
            self.size = 0;
        }
        self.file.write_all(&bytes)?;
        self.size += bytes.len() as u64;
        self.next = expected;
        Ok(())
    }
}

/// 造会话没成（`session.created` 没落盘）时收拾会话目录：里面只有一段空的第一段（分叉出来的第一段照分叉点起名，施工 C-8
/// 上），才连目录一起删掉，交回删了没有；有别的东西的，一个字节不动（施工 4-9 再补四下）。
///
/// # Errors
///
/// 读不了目录、删不掉。
pub fn abandon(dir: &Path) -> io::Result<bool> {
    let entries = fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    let [only] = entries.as_slice() else {
        return Ok(false);
    };
    let named = only
        .file_name()
        .to_str()
        .and_then(|name| name.strip_suffix(".jsonl"))
        .is_some_and(|stem| stem.len() == 12 && stem.bytes().all(|byte| byte.is_ascii_digit()));
    let empty_first = named && only.file_type()?.is_file() && only.metadata()?.len() == 0;
    if !empty_first {
        return Ok(false);
    }
    fs::remove_file(only.path())?;
    fs::remove_dir(dir)?;
    Ok(true)
}

/// 段文件的名字：这一段第一条的序号，补零到 12 位（07 第三节「段怎么存」）。
fn segment_name(first: Seq) -> String {
    segment_name_of(first.get())
}

/// 同 [`segment_name`]，照序号的数字。
fn segment_name_of(first: u64) -> String {
    format!("{first:012}.jsonl")
}

/// 建一段新的，只许新建；建好了同步所在目录，新文件本身才算落盘。
fn new_segment(dir: &Path, first: Seq) -> io::Result<File> {
    let file = OpenOptions::new()
        .append(true)
        .create_new(true)
        .open(dir.join(segment_name(first)))?;
    sync_dir(dir)?;
    Ok(file)
}

#[cfg(test)]
mod tests;
