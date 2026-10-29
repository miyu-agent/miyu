//! 打开会话日志时的自检（`docs/designs/07-存储.md` 第四节「打开日志时自检三件事」）：最后一行完整、
//! 每一行都能解析、序号连续。只截最后一段末尾那半行：它一定从未被确认过。别的不对一律报错，写明
//! 是哪一段第几行，不自动修：日志是真相，修错了就是丢了。

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use miyu_kernel::event::Event;

use super::{Mark, SessionLog};

mod parse;
mod walk;

pub use parse::Flow;

use walk::{Piece, walk};

/// 打开不了会话日志。
#[derive(Debug)]
pub enum OpenError {
    /// 没有这个会话：目录不存在，或者里面一段日志都没有。
    Missing(PathBuf),
    /// 日志坏了：哪一段、第几行（从 1 数）、怎么坏的。
    Broken {
        /// 哪一段。
        segment: PathBuf,
        /// 第几行。
        line: usize,
        /// 怎么坏的。
        why: String,
    },
    /// 读写出错。
    Io(io::Error),
}

/// 写进运行日志的那一句：不带目录（带着会话编号），不带怎么坏的（可能转述那一行的字），段只写文件名（施工 LG-1 上）。要细的看
/// 字段。
impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::Missing(_) => f.write_str("no session log"),
            OpenError::Broken { segment, line, .. } => {
                let name = segment.file_name().unwrap_or_default();
                write!(f, "{} line {line} is broken", name.to_string_lossy())
            }
            OpenError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<io::Error> for OpenError {
    fn from(error: io::Error) -> OpenError {
        OpenError::Io(error)
    }
}

impl SessionLog {
    /// 打开一个会话的日志：照段的先后一行行读，自检，截掉最后一段末尾那半行。返回开着的日志，
    /// 和读出来的事件（交给内核载入，`02-内核.md` 第六节「载入、崩溃、重启」）。空的段当没有；
    /// 最后一段是空的，接着往里写。
    ///
    /// # Errors
    ///
    /// 没有这个会话；日志坏了；读写出错。
    pub fn open(dir: &Path, limit: u64) -> Result<(SessionLog, Vec<Event>), OpenError> {
        let mut events = Vec::new();
        let log = SessionLog::open_each(dir, limit, |read| {
            gather(&mut events, read);
            Flow::Go
        })?;
        Ok((log, events))
    }

    /// 同 [`SessionLog::open`]，只是读出来的事件照先后一块块交给 `each`，不攒成一整份（施工 V-15：载入时边读边重放，
    /// 前面的用完就放掉）。`each` 交回 [`Flow::Stop`] 的不读下去，交回没有这个会话。
    ///
    /// # Errors
    ///
    /// 同 [`SessionLog::open`]；`each` 叫停了的是没有这个会话。
    pub fn open_each(
        dir: &Path,
        limit: u64,
        mut each: impl FnMut(Vec<Event>) -> Flow,
    ) -> Result<SessionLog, OpenError> {
        let mut stopped = false;
        let end = walk(dir, HalfLine::Cut, None, &mut |piece| match piece {
            Piece::Events(read) => {
                let flow = each(read);
                stopped = matches!(flow, Flow::Stop);
                flow
            }
            Piece::SegmentEnd => Flow::Go,
        })?
        .filter(|_| !stopped)
        .ok_or_else(|| OpenError::Missing(dir.to_path_buf()))?;
        let last = dir.join(super::segment_name_of(end.segment));
        let size = fs::metadata(&last)?.len();
        let file = OpenOptions::new().append(true).open(&last)?;
        Ok(SessionLog {
            dir: dir.to_path_buf(),
            file,
            segment: end.segment,
            size,
            next: end.next,
            limit,
        })
    }
}

/// 只读地读整份会话日志（施工 3-9 下）：和 [`SessionLog::open`] 一样自检，只是最后一段末尾没写完的半行
/// 跳过、不截，一个字节都不写：会话可能正在往里写。测试盯着一个在跑的会话时用它；[`SessionLog::open`]
/// 会截掉正在写的那半行，把活的日志写坏。
///
/// # Errors
///
/// 没有这个会话（目录没有，或者一段都还没有）；日志坏了；读写出错。
pub fn read_events(dir: &Path) -> Result<Vec<Event>, OpenError> {
    let mut events = Vec::new();
    walk(dir, HalfLine::Skip, None, &mut |piece| {
        if let Piece::Events(read) = piece {
            gather(&mut events, read);
        }
        Flow::Go
    })?
    .ok_or_else(|| OpenError::Missing(dir.to_path_buf()))?;
    Ok(events)
}

/// 同 [`read_events`]，只是读一段交一段给 `each`，它交回 `false` 就不读下去（施工 6-4：`history` 翻长会话，叫停了
/// 不用读完整份）。自检照旧，一个字节都不写。
///
/// # Errors
///
/// 同 [`read_events`]。
pub fn read_segments(dir: &Path, each: impl FnMut(Vec<Event>) -> bool) -> Result<(), OpenError> {
    walk(dir, HalfLine::Skip, None, &mut by_segment(each)).map(|_| ())
}

/// 同 [`read_segments`]，只是从 `from` 读起（施工 3-8 七补：列会话照索引记的位置，只读多出来的那一截），交回读到了哪里：
/// 最后一段、照到的整行末尾、下一条该是几号。`from` 是空的从头读。
///
/// `from` 和日志对不上的交回空的，一条都不交给 `each`：记的那一段没有了、比记的短了、记的位置前面一个字节不是 `\n`（不在
/// 一行的开头）。对得上的照旧自检，只是从 `from` 读起的那一段，报坏了时的行号从 `from` 数起。
///
/// # Errors
///
/// 同 [`read_events`]。
pub fn read_marked(
    dir: &Path,
    from: Option<&Mark>,
    each: impl FnMut(Vec<Event>) -> bool,
) -> Result<Option<Mark>, OpenError> {
    walk(dir, HalfLine::Skip, from, &mut by_segment(each))
}

/// 最后一段末尾没写完的半行怎么办。
#[derive(Clone, Copy, PartialEq, Eq)]
enum HalfLine {
    /// 截掉：载入以后接着往里写。
    Cut,
    /// 跳过、不动：只读。
    Skip,
}

/// 一块块收进一整份：头一块整个搬过来，不再拷一遍（施工 V-14）。
fn gather(events: &mut Vec<Event>, read: Vec<Event>) {
    if events.is_empty() {
        *events = read;
    } else {
        events.extend(read);
    }
}

/// 照段收：一段读完了才把这一段的事件一起交给 `each`，它交回 `false` 就停（同施工 V-15 以前）。
fn by_segment(mut each: impl FnMut(Vec<Event>) -> bool) -> impl FnMut(Piece) -> Flow {
    let mut segment = Vec::new();
    move |piece| match piece {
        Piece::Events(read) => {
            gather(&mut segment, read);
            Flow::Go
        }
        Piece::SegmentEnd => match each(std::mem::take(&mut segment)) {
            true => Flow::Go,
            false => Flow::Stop,
        },
    }
}

/// 只读地拿会话日志的第一条（`session.created`）：列出会话时用（施工 3-9 下）。只读第一段开头那一行，
/// 不截、不写：会话可能正在往最后一段里写。第一条落了盘，会话才算造好，所以它总是完整的一行；
/// 还没写完的当没有这个会话。
///
/// # Errors
///
/// 没有这个会话；第一行读不懂；读写出错。
pub fn first_event(dir: &Path) -> Result<Event, OpenError> {
    let segments = segments(dir)?;
    let Some((_, first)) = segments.first() else {
        return Err(OpenError::Missing(dir.to_path_buf()));
    };
    let mut line = String::new();
    BufReader::new(fs::File::open(first)?).read_line(&mut line)?;
    let Some(line) = line.strip_suffix('\n') else {
        return Err(OpenError::Missing(dir.to_path_buf()));
    };
    Event::from_line(line).map_err(|error| broken(first, 1, error.to_string()))
}

/// 目录里的段：名字是 12 位数字加 `.jsonl` 的，照数字排。别的文件不看。
fn segments(dir: &Path) -> Result<Vec<(u64, PathBuf)>, OpenError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpenError::Missing(dir.to_path_buf()));
        }
        Err(error) => return Err(error.into()),
    };
    let mut segments = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let first = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".jsonl"))
            .filter(|stem| stem.len() == 12 && stem.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|stem| stem.parse::<u64>().ok());
        if let Some(first) = first {
            segments.push((first, path));
        }
    }
    segments.sort();
    Ok(segments)
}

fn broken(segment: &Path, line: usize, why: String) -> OpenError {
    OpenError::Broken {
        segment: segment.to_path_buf(),
        line,
        why,
    }
}
