//! 照段的先后读、自检（`docs/designs/07-存储.md` 第四节）：一段里解好一块交一块（施工 V-15），交的是 [`Piece`]：
//! 一块事件，或者一段读完了。照段收齐了再用的（`read_segments`、`read_marked`）在一段读完时一起交，交出去的和以前
//! 一样；载入（`SessionLog::open_each`）一块块收，前面的用完就放掉。

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use super::parse::{self, Flow};
use super::{HalfLine, Mark, OpenError, broken, segments};

/// 交给收的一方的一样。
pub(super) enum Piece {
    /// 照先后的一块事件，序号接着上一块。
    Events(Vec<Event>),
    /// 一段读完了，这一段的事件都交了。
    SegmentEnd,
}

/// 照段的先后读、自检，一块块交给 `sink`，它交回 [`Flow::Stop`] 就停。`from` 是空的从第一段开头读，不是空的从它记的
/// 那一段、那个字节读起（`read_marked`）。交回读到了哪里（停下的，是停在读完了的最后一段后面）；`from` 和日志对不上的
/// 交回空的。
pub(super) fn walk(
    dir: &Path,
    half: HalfLine,
    from: Option<&Mark>,
    sink: &mut dyn FnMut(Piece) -> Flow,
) -> Result<Option<Mark>, OpenError> {
    let segments = segments(dir)?;
    if segments.is_empty() {
        return Err(OpenError::Missing(dir.to_path_buf()));
    }
    let (start, mut offset, mut next) = match from {
        // 从第一段读起：第一条照第一段的名字（分叉出来的会话接着分叉点往下数，施工 C-8 上），不是 1。
        None => (0, 0, Seq::new(segments[0].0).unwrap_or(Seq::FIRST)),
        Some(mark) => match segments
            .iter()
            .position(|(first, _)| *first == mark.segment)
        {
            Some(k) => (k, mark.bytes, mark.next),
            None => return Ok(None),
        },
    };
    let mut end = Mark {
        segment: segments[start].0,
        bytes: offset,
        next,
    };
    for (k, (first, path)) in segments.iter().enumerate().skip(start) {
        let segment = Segment {
            path,
            first: *first,
            from: offset,
            is_last: k + 1 == segments.len(),
        };
        offset = 0;
        let Some((bytes, stopped)) = read_segment(&segment, &mut next, half, sink)? else {
            return Ok(None);
        };
        if stopped {
            break;
        }
        end = Mark {
            segment: *first,
            bytes,
            next,
        };
        if matches!(sink(Piece::SegmentEnd), Flow::Stop) {
            break;
        }
    }
    Ok(Some(end))
}

/// 要读的一段。
struct Segment<'a> {
    /// 文件。
    path: &'a Path,
    /// 名字：这一段第一条的序号。
    first: u64,
    /// 从第几个字节读起。
    from: u64,
    /// 是最后一段。
    is_last: bool,
}

/// 读一段，从第 `from` 个字节读起：每一行读成事件，序号要接着 `next`，一块块交给 `sink`。最后一段末尾没写完的半行照
/// `half` 截掉或者跳过；别的段末尾有半行，报错。交回照到的整行末尾、`sink` 叫停了没有；`from` 前面一个字节不是 `\n` 的
/// （这一段比 `from` 短的也是），交回空的（对不上）。
fn read_segment(
    segment: &Segment<'_>,
    next: &mut Seq,
    half: HalfLine,
    sink: &mut dyn FnMut(Piece) -> Flow,
) -> Result<Option<(u64, bool)>, OpenError> {
    let Segment {
        path,
        first,
        from,
        is_last,
    } = *segment;
    let mut file = fs::File::open(path)?;
    // 从一行中间读起的不算数：多读前面那一个字节，看它是不是 `\n`。这一段比记的短了的，那个字节读不到，也算对不上。
    let skip = u64::from(from > 0);
    file.seek(SeekFrom::Start(from - skip))?;
    // 照文件的大小一次留够（施工 V-12）：原来一路翻倍长大，十兆的日志多搬好几遍。读的时候还在往后写的，照样读到末尾。
    let size = file.metadata()?.len().saturating_sub(from - skip);
    let mut read = Vec::with_capacity(usize::try_from(size).unwrap_or(0).saturating_add(1));
    file.read_to_end(&mut read)?;
    if skip == 1 && read.first() != Some(&b'\n') {
        return Ok(None);
    }
    let bytes = &read[usize::from(skip == 1)..];
    let complete = bytes
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |at| at + 1);
    let end = from + complete as u64;
    if complete < bytes.len() {
        if !is_last {
            return Err(broken(
                path,
                memchr::memchr_iter(b'\n', &bytes[..complete]).count() + 1,
                "ends in a partial line, yet more segments follow".to_string(),
            ));
        }
        if half == HalfLine::Cut {
            truncate(path, end)?;
        }
    }
    // 从一段中间读起的，读到的第一条不是这一段的第一条：名字不在这里查。
    let whole = from == 0;
    let mut checked = 0;
    let mut wrong = None;
    let mut misnamed = None;
    let mut stopped = false;
    let bad = parse::stream(&bytes[..complete], |events| {
        // 先照先后查序号，接不上的当场停；段名不对的记下，照以前的先后最后才报（序号、读不成的那一行在它前面）。
        for event in &events {
            checked += 1;
            if event.seq != *next {
                wrong = Some((checked, format!("seq should be {next}, got {}", event.seq)));
                return Flow::Stop;
            }
            if whole && checked == 1 && event.seq.get() != first {
                misnamed = Some((
                    1,
                    format!("the segment is named {first} but starts with {}", event.seq),
                ));
            }
            *next = next.next();
        }
        if misnamed.is_some() {
            return Flow::Go;
        }
        let flow = sink(Piece::Events(events));
        stopped = matches!(flow, Flow::Stop);
        flow
    });
    if let Some((number, why)) = wrong.or(bad).or(misnamed) {
        return Err(broken(path, number, why));
    }
    if whole && is_last && checked == 0 && !stopped && first != next.get() {
        return Err(broken(
            path,
            1,
            format!("the empty last segment is named {first} but the next event is {next}"),
        ));
    }
    Ok(Some((end, stopped)))
}

/// 截到 `len` 字节，再同步。
fn truncate(path: &Path, len: u64) -> io::Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(len)?;
    file.sync_all()
}
