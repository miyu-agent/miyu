//! 只数行、不解（施工 V-41 上，`docs/construction/V-41上-翻页不整份读.md`）：翻页要先知道每一轮从哪一行起、派出去的任务记在
//! 哪几行，再只解要的那一截。一段的名字就是这一段第一条的序号、一行一条、序号连着（`07-存储.md` 第四节，打开时自检过），
//! 所以一行的序号照段名和第几行就算得出，不用解；交出去的是每一行的原文和位置，要不要解、解哪几行归调的一方。
//!
//! **数过去的行不自检**（同 [`super::read_range`] 定位跳过的那一截）：不解、不查序号；只查数得出来的那几样：不是最后一段的
//! 末尾不许有半行，有行的段名字要接着前面的行数，空着的最后一段名字要是下一条的序号。真解了的行（[`Line::event`]、
//! [`super::read_between`]、[`super::read_line_near`]）照旧查读不读得成、序号对不对。

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use super::parse::Flow;
use super::range::{after_lines, line_at};
use super::walk::{
    Check, Piece, WINDOW as READ_WINDOW, count_lines, newline_at, read_windows, whole_lines_end,
};
use super::{Mark, OpenError, Parse, broken, segments};

/// 一窗读多少字节：只数行，一个线程，窗口大了不更快。一行比它长的照这一行撑大。
const WINDOW: usize = 1024 * 1024;

/// 数到的一行。
pub struct Line<'a> {
    /// 这一行的序号：照段名和第几行算的，没解、没对过。
    pub seq: Seq,
    /// 这一行的开头：哪一段、第几个字节，`next` 是这一行的序号。交给 [`super::read_between`]、[`super::read_line_near`]
    /// 从这里读起。
    pub start: Mark,
    /// 这一行的原文，不带末尾的 `\n`。
    pub text: &'a [u8],
    /// 这一段的文件：解坏了的报哪一段。
    path: &'a Path,
}

impl Line<'_> {
    /// 这一行后面那一行的开头（同一段里，可以是这一段的末尾）：[`super::read_between`] 读到这一行为止就交它。
    #[must_use]
    pub fn end(&self) -> Mark {
        Mark {
            segment: self.start.segment,
            bytes: self.start.bytes + self.text.len() as u64 + 1,
            next: self.seq.next(),
        }
    }

    /// 解这一行（[`Event::from_line`]），解出来的序号要是数出来的那个。
    ///
    /// # Errors
    ///
    /// 读不成、序号对不上的报日志坏了：哪一段、第几行（照那一段的开头数）、为什么，写法同从头读时报的。
    pub fn event(&self) -> Result<Event, OpenError> {
        parsed(self.path, self.start.segment, self.seq, self.text)
    }
}

/// 解第 `seq` 条（段 `segment` 里的一行 `text`），对一遍序号。
fn parsed(path: &Path, segment: u64, seq: Seq, text: &[u8]) -> Result<Event, OpenError> {
    let number = usize::try_from(seq.get().saturating_sub(segment)).map_or(usize::MAX, |n| n + 1);
    let event = std::str::from_utf8(text)
        .map_err(|_| broken(path, number, "not UTF-8".to_string()))
        .and_then(|text| {
            Event::from_line(text)
                .map_err(|error| broken(path, number, format!("not readable: {error}")))
        })?;
    if event.seq != seq {
        return Err(broken(
            path,
            number,
            format!("seq should be {seq}, got {}", event.seq),
        ));
    }
    Ok(event)
}

/// 照段的先后数每一整行，序号小于 `before` 的（`before` 是空的数到最后）一行行交给 `each`，它交回 [`Flow::Stop`] 就停。
/// 最后一段末尾没写完的半行跳过，一个字节都不写。
///
/// # Errors
///
/// 没有这个会话；不是最后一段的末尾有半行、有行的段名字接不上前面的行数、空着的最后一段名字不是下一条的序号（这几样从头
/// 读也报）；读写出错。
pub fn scan(
    dir: &Path,
    before: Option<u64>,
    each: &mut dyn FnMut(Line<'_>) -> Flow,
) -> Result<(), OpenError> {
    let segments = segments(dir)?;
    if segments.is_empty() {
        return Err(OpenError::Missing(dir.to_path_buf()));
    }
    let mut next = Seq::new(segments[0].0).unwrap_or(Seq::FIRST);
    let mut buffer = Vec::new();
    for (k, (first, path)) in segments.iter().enumerate() {
        if before.is_some_and(|before| next.get() >= before) {
            break;
        }
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        if metadata.is_dir() {
            // 名字像段的目录：照从头读报系统说的那个错。
            let _ = file.read(&mut [0; 1])?;
        }
        let len = metadata.len();
        let end = whole_lines_end(&mut file, 0, len)?;
        if end < len && k + 1 < segments.len() {
            return Err(broken(
                path,
                count_lines(&mut file, 0, end)? + 1,
                "ends in a partial line, yet more segments follow".to_string(),
            ));
        }
        if end == 0 {
            if k + 1 == segments.len() && *first != next.get() {
                return Err(broken(
                    path,
                    1,
                    format!("the empty last segment is named {first} but the next event is {next}"),
                ));
            }
            continue;
        }
        if *first != next.get() {
            return Err(broken(
                path,
                1,
                format!("the segment is named {first} but the next event is {next}"),
            ));
        }
        let segment = Counted {
            file: &mut file,
            path,
            first: *first,
            end,
        };
        if count(segment, &mut next, before, &mut buffer, each)? {
            break;
        }
    }
    Ok(())
}

/// 数着的一段：到整行的末尾 `end` 为止。
struct Counted<'a> {
    file: &'a mut File,
    path: &'a Path,
    first: u64,
    end: u64,
}

/// 一窗窗数一段的整行，交给 `each`；交回停没停（叫停了，或者到了 `before`）。
fn count(
    segment: Counted<'_>,
    next: &mut Seq,
    before: Option<u64>,
    buffer: &mut Vec<u8>,
    each: &mut dyn FnMut(Line<'_>) -> Flow,
) -> io::Result<bool> {
    let Counted {
        file,
        path,
        first,
        end,
    } = segment;
    file.seek(SeekFrom::Start(0))?;
    if buffer.len() < WINDOW {
        buffer.resize(WINDOW, 0);
    }
    // `buffer` 开头 `held` 个字节是上一窗余下的半行，它从文件的第 `base` 个字节起。
    let (mut held, mut base, mut left) = (0, 0u64, end);
    while left > 0 {
        if held == buffer.len() {
            let longer = buffer.len() * 2;
            buffer.resize(longer, 0);
        }
        let more = (buffer.len() - held).min(usize::try_from(left).unwrap_or(usize::MAX));
        file.read_exact(&mut buffer[held..held + more])?;
        left -= more as u64;
        let filled = held + more;
        let mut start = 0;
        for newline in memchr::memchr_iter(b'\n', &buffer[held..filled]) {
            let newline = held + newline;
            let seq = *next;
            if before.is_some_and(|before| seq.get() >= before) {
                return Ok(true);
            }
            let line = Line {
                seq,
                start: Mark {
                    segment: first,
                    bytes: base + start as u64,
                    next: seq,
                },
                text: &buffer[start..newline],
                path,
            };
            *next = seq.next();
            start = newline + 1;
            if matches!(each(line), Flow::Stop) {
                return Ok(true);
            }
        }
        buffer.copy_within(start..filled, 0);
        held = filled - start;
        base += start as u64;
    }
    Ok(false)
}

/// 照 `near`（同一段里第 `seq` 条或者它前面的一行的开头）数换行找到第 `seq` 条，解它，对一遍序号。
pub(super) fn line_near(dir: &Path, near: &Mark, seq: Seq) -> Result<Option<Event>, OpenError> {
    let Some(skip) = seq
        .get()
        .checked_sub(near.next.get())
        .and_then(|skip| usize::try_from(skip).ok())
    else {
        return Ok(None);
    };
    let path = dir.join(super::super::segment_name_of(near.segment));
    let mut file = match OpenOptions::new().read(true).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if near.bytes > 0 {
        let mut byte = [0];
        file.seek(SeekFrom::Start(near.bytes - 1))?;
        if file.read(&mut byte)? != 1 || byte[0] != b'\n' {
            return Ok(None);
        }
    }
    let Some(offset) = after_lines(&mut file, skip)? else {
        return Ok(None);
    };
    let Some(text) = line_at(&mut file, near.bytes + offset)? else {
        return Ok(None);
    };
    parsed(&path, near.segment, seq, &text).map(Some)
}

/// 读 `from` 到 `to` 之间的整行（施工 V-41 上，[`super::read_between`]）：两样都是一行的开头，`to` 是要的最后一行后面那一行
/// 的开头（可以在后面的段里，可以是一段的末尾）。每一行照 `how` 解，序号从 `from.next` 起要连着，一块块交给 `sink`，它交回
/// [`Flow::Stop`] 就停。从一段开头读起的照旧查段名；不查 `to` 以后的。`from` 那一段没有了、`from`、`to` 不在一行的开头，
/// 是读的时候日志变了（只追加，不该有），报读写出错。
pub(super) fn between(
    dir: &Path,
    from: &Mark,
    to: &Mark,
    how: Parse<'_>,
    sink: &mut dyn FnMut(Piece) -> Flow,
) -> Result<(), OpenError> {
    let segments = segments(dir)?;
    if segments.is_empty() {
        return Err(OpenError::Missing(dir.to_path_buf()));
    }
    let Some(k) = segments
        .iter()
        .position(|(first, _)| *first == from.segment)
    else {
        return Err(changed().into());
    };
    let mut next = from.next;
    for (first, path) in &segments[k..] {
        if *first > to.segment {
            break;
        }
        let start = if *first == from.segment {
            from.bytes
        } else {
            0
        };
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        let stop = if *first == to.segment {
            to.bytes
        } else {
            whole_lines_end(&mut file, start, len)?
        };
        let lined_up = |file: &mut File, at: u64| -> io::Result<bool> {
            Ok(at == 0 || newline_at(file, at - 1, len)?)
        };
        if stop < start || !lined_up(&mut file, start)? || !lined_up(&mut file, stop)? {
            return Err(changed().into());
        }
        // 从一段中间读起的，前面跳过了几行：序号照段名数（打开时自检过），报坏了的行号照那一段的开头数。
        let skipped = usize::try_from(next.get().saturating_sub(*first)).unwrap_or(usize::MAX);
        let mut check = Check {
            first: *first,
            whole: start == 0,
            next: &mut next,
            checked: 0,
            wrong: None,
            misnamed: None,
            stopped: false,
        };
        let bad = read_windows(&mut file, start..stop, (READ_WINDOW, how), &mut check, sink)?;
        if let Some((number, why)) = check.wrong.take().or(bad).or(check.misnamed.take()) {
            let number = if start == 0 { number } else { skipped + number };
            return Err(broken(path, number, why));
        }
        if check.stopped {
            break;
        }
    }
    Ok(())
}

/// 读的时候日志变了。
fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "the segment changed while it was read",
    )
}

#[cfg(test)]
mod tests;
