//! 照序号读一截（施工 V-37 中，[`super::read_range`]）：不另存索引，照日志的规矩数换行定位。一段的名字就是这一段第一条
//! 的序号、一行一条、序号连着（`07-存储.md` 第四节，打开时自检过），第 `from` 条在名字不大于它的最后一段的第
//! `from − 名字 + 1` 行。定位只数换行，不解前面的行；对不上的退回从头读。

use std::fs::File;
use std::io::{self, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use super::parse::Flow;
use super::walk::{Piece, SCAN, fill, walk};
use super::{HalfLine, Mark, OpenError, segments};

/// 读第 `from` 条到第 `upto` 条，一块块交给 `each`（同 [`super::read_range`]）。
pub(super) fn read(
    dir: &Path,
    from: Seq,
    upto: Option<Seq>,
    each: &mut dyn FnMut(Vec<Event>) -> Flow,
) -> Result<(), OpenError> {
    let all = segments(dir)?;
    if all.is_empty() {
        return Err(OpenError::Missing(dir.to_path_buf()));
    }
    if upto.is_some_and(|upto| upto < from) {
        return Ok(());
    }
    let mut sink = between(from, upto, each);
    match place(&all, from)? {
        Place::Nothing => Ok(()),
        Place::Start => walk(dir, HalfLine::Skip, None, &mut sink).map(drop),
        Place::At {
            mark,
            path,
            skipped,
        } => match walk(dir, HalfLine::Skip, Some(&mark), &mut sink) {
            Ok(Some(_)) => Ok(()),
            // 定位以后这一段又变了（日志只追加，不该有）：一条都还没交，退回从头读。
            Ok(None) => walk(dir, HalfLine::Skip, None, &mut sink).map(drop),
            // 从这一段中间读起的，行号照这一段开头数（同从头读）。
            Err(OpenError::Broken { segment, line, why }) if segment == path => {
                Err(OpenError::Broken {
                    segment,
                    line: line + skipped,
                    why,
                })
            }
            Err(error) => Err(error),
        },
    }
}

/// 只交 `from..=upto` 的，读过 `upto` 就叫停。
fn between<'a>(
    from: Seq,
    upto: Option<Seq>,
    each: &'a mut dyn FnMut(Vec<Event>) -> Flow,
) -> impl FnMut(Piece) -> Flow + 'a {
    move |piece| {
        let Piece::Events(mut events) = piece else {
            return Flow::Go;
        };
        // 一块里序号连着：前面的、后面的各一刀。
        let early = events.partition_point(|event| event.seq < from);
        events.drain(..early);
        let past = upto.is_some_and(|upto| events.last().is_some_and(|event| event.seq >= upto));
        if let Some(upto) = upto {
            events.truncate(events.partition_point(|event| event.seq <= upto));
        }
        if !events.is_empty() && matches!(each(events), Flow::Stop) {
            return Flow::Stop;
        }
        if past { Flow::Stop } else { Flow::Go }
    }
}

/// 第 `from` 条在哪。
enum Place {
    /// 从头读：`from` 在第一段前面，或者对不上。
    Start,
    /// 最后一段里整行都在 `from` 前面：还没有，什么都不交。
    Nothing,
    /// 在这一段的这个字节起（前面跳过了 `skipped` 行）。
    At {
        mark: Mark,
        path: PathBuf,
        skipped: usize,
    },
}

/// 找第 `from` 条：名字不大于它的最后一段，数 `from − 名字` 个换行，那一行读出来要是第 `from` 条。那一段没有那么多整行的：
/// 是最后一段的什么都不交，不是的（空段、行数不够）对不上；那一行读不成、不是第 `from` 条的也对不上。
fn place(segments: &[(u64, PathBuf)], from: Seq) -> io::Result<Place> {
    let Some(k) = segments.iter().rposition(|(first, _)| *first <= from.get()) else {
        return Ok(Place::Start);
    };
    let (first, path) = &segments[k];
    let is_last = k + 1 == segments.len();
    let behind = if is_last {
        Place::Nothing
    } else {
        Place::Start
    };
    let Ok(skip) = usize::try_from(from.get() - first) else {
        return Ok(behind);
    };
    let mut file = File::open(path)?;
    let Some(offset) = after_lines(&mut file, skip)? else {
        return Ok(behind);
    };
    let Some(line) = line_at(&mut file, offset)? else {
        return Ok(behind);
    };
    let lines_up = std::str::from_utf8(&line)
        .ok()
        .and_then(|text| Event::from_line(text).ok())
        .is_some_and(|event| event.seq == from);
    if !lines_up {
        return Ok(Place::Start);
    }
    Ok(Place::At {
        mark: Mark {
            segment: *first,
            bytes: offset,
            next: from,
        },
        path: path.clone(),
        skipped: skip,
    })
}

/// 第 `skip` 个换行后面那个字节（第 `skip + 1` 行的开头）：一块块流过去数，不解、不留。换行不够 `skip` 个的没有。
fn after_lines(file: &mut File, skip: usize) -> io::Result<Option<u64>> {
    if skip == 0 {
        return Ok(Some(0));
    }
    let mut block = vec![0; SCAN];
    let mut at = 0;
    let mut seen = 0;
    loop {
        let got = fill(file, &mut block)?;
        if got == 0 {
            return Ok(None);
        }
        let here = memchr::memchr_iter(b'\n', &block[..got]).count();
        if seen + here >= skip {
            let newline = memchr::memchr_iter(b'\n', &block[..got])
                .nth(skip - seen - 1)
                .unwrap_or(got - 1);
            return Ok(Some(at + newline as u64 + 1));
        }
        seen += here;
        at += got as u64;
    }
}

/// 从第 `at` 个字节起的一整行（不带 `\n`）：到文件末尾也没有 `\n` 的（半行、什么都没有）没有。
fn line_at(file: &mut File, at: u64) -> io::Result<Option<Vec<u8>>> {
    file.seek(SeekFrom::Start(at))?;
    let mut line = Vec::new();
    let mut block = vec![0; SCAN];
    loop {
        let got = fill(file, &mut block)?;
        if got == 0 {
            return Ok(None);
        }
        if let Some(newline) = memchr::memchr(b'\n', &block[..got]) {
            line.extend_from_slice(&block[..newline]);
            return Ok(Some(line));
        }
        line.extend_from_slice(&block[..got]);
    }
}

#[cfg(test)]
mod tests;
