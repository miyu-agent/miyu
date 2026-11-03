//! 照段的先后读、自检（`docs/designs/07-存储.md` 第四节）：一段照定长的窗口一窗窗读（施工 V-37 中：原文不再整段同时在
//! 内存里），一窗里解好一块交一块（施工 V-15），交的是 [`Piece`]：一块事件，或者一段读完了。照段收齐了再用的
//! （`read_segments`、`read_marked`）在一段读完时一起交，交出去的和以前一样；载入（`SessionLog::open_each`）一块块收，
//! 前面的用完就放掉。
//!
//! 交出去的事件、先后、报错（哪一段、第几行、为什么、几种错的先后）、读到了哪里、截半行以后的文件，都和以前整段读进来
//! 再解的一样（测试里留着以前那一份当准绳）：先照打开那一刻的长度从后往前找出整行的末尾，半行该报的先报、该截的先截，
//! 再一窗窗读、查每一行。

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::mpsc::{self, SyncSender};

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use super::parse::{self, Flow};
use super::{HalfLine, Mark, OpenError, Parse, broken, segments};

/// 一窗读多少字节（施工 V-37 中，量过定的）：一段的原文最多这么多同时在内存里，一行比它还长的照这一行撑大。一个线程至少
/// 分半 MiB、最多 8 个线程（`parse.rs`），4 MiB 是 8 个线程都分得到活的最小的窗口：1 MiB 的打开大会话慢了快一倍；再大只多
/// 占内存，16 MiB 不更快。
const WINDOW: usize = 4 * 1024 * 1024;

/// 从后往前找整行的末尾、数换行时一次读多少。
pub(super) const SCAN: usize = 64 * 1024;

/// 交给收的一方的一样。
pub(super) enum Piece {
    /// 照先后的一块事件，序号接着上一块。
    Events(Vec<Event>),
    /// 一段读完了，这一段的事件都交了。
    SegmentEnd,
}

/// 照段的先后读、自检，每一行照 `how` 解（施工 V-39 上补），一块块交给 `sink`，它交回 [`Flow::Stop`] 就停。`from` 是空的
/// 从第一段开头读，不是空的从它记的那一段、那个字节读起（`read_marked`）。交回读到了哪里（停下的，是停在读完了的最后一段
/// 后面）；`from` 和日志对不上的交回空的。
pub(super) fn walk(
    dir: &Path,
    half: HalfLine,
    from: Option<&Mark>,
    how: Parse<'_>,
    sink: &mut dyn FnMut(Piece) -> Flow,
) -> Result<Option<Mark>, OpenError> {
    walk_in(dir, (half, how), from, WINDOW, sink)
}

/// 同 [`walk`]，一窗读 `window` 字节（测试里调小，逼出窗口边上的几种）。
fn walk_in(
    dir: &Path,
    (half, how): (HalfLine, Parse<'_>),
    from: Option<&Mark>,
    window: usize,
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
        let Some((bytes, stopped)) = read_segment(&segment, &mut next, (half, how), window, sink)?
        else {
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
///
/// 只读到打开这一刻的末尾（施工 V-37 中）：读的时候还在往后写的，后写的不要。以前整段读到读那一刻的末尾，两样都是这个
/// 只追加的文件的一个前缀。
fn read_segment(
    segment: &Segment<'_>,
    next: &mut Seq,
    (half, how): (HalfLine, Parse<'_>),
    window: usize,
    sink: &mut dyn FnMut(Piece) -> Flow,
) -> Result<Option<(u64, bool)>, OpenError> {
    let Segment {
        path,
        first,
        from,
        is_last,
    } = *segment;
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.is_dir() {
        // 名字像段的目录：照以前读一下，报系统说的那个错（Windows 上打开时就报了）。
        let _ = file.read(&mut [0; 1])?;
    }
    let len = metadata.len();
    // 从一行中间读起的不算数：前面那一个字节要是 `\n`。这一段比记的短了的，那个字节读不到，也算对不上。
    if from > 0 && !newline_at(&mut file, from - 1, len)? {
        return Ok(None);
    }
    // 半行先报、先截，再查每一行（同以前整段读进来时的先后）：这一段里前面有坏行的，照样先报半行、先截掉。
    let end = whole_lines_end(&mut file, from, len)?;
    if end < len {
        if !is_last {
            return Err(broken(
                path,
                count_lines(&mut file, from, end)? + 1,
                "ends in a partial line, yet more segments follow".to_string(),
            ));
        }
        if half == HalfLine::Cut {
            truncate(path, end)?;
        }
    }
    let mut check = Check {
        first,
        whole: from == 0,
        next,
        checked: 0,
        wrong: None,
        misnamed: None,
        stopped: false,
    };
    let bad = read_windows(&mut file, from..end, (window, how), &mut check, sink)?;
    if let Some((number, why)) = check.wrong.take().or(bad).or(check.misnamed.take()) {
        return Err(broken(path, number, why));
    }
    let Check {
        whole,
        next,
        checked,
        stopped,
        ..
    } = check;
    if whole && is_last && checked == 0 && !stopped && first != next.get() {
        return Err(broken(
            path,
            1,
            format!("the empty last segment is named {first} but the next event is {next}"),
        ));
    }
    Ok(Some((end, stopped)))
}

/// 一段里一路查下来的，跨窗接着。
struct Check<'a> {
    /// 这一段的名字。
    first: u64,
    /// 从这一段开头读起的：名字在这里查。从一段中间读起的，读到的第一条不是这一段的第一条。
    whole: bool,
    /// 下一条该是几号。
    next: &'a mut Seq,
    /// 查过几行，从读起的地方数。
    checked: usize,
    /// 序号接不上的那一行。
    wrong: Option<(usize, String)>,
    /// 段名不对：照以前的先后最后才报。
    misnamed: Option<(usize, String)>,
    /// 收的一方叫停了。
    stopped: bool,
}

impl Check<'_> {
    /// 解好的一块：先照先后查序号，接不上的当场停；段名不对的记下，照以前的先后最后才报（序号、读不成的那一行在它前面），
    /// 不再往下交。
    fn block(&mut self, events: Vec<Event>, sink: &mut dyn FnMut(Piece) -> Flow) -> Flow {
        for event in &events {
            self.checked += 1;
            if event.seq != *self.next {
                self.wrong = Some((
                    self.checked,
                    format!("seq should be {}, got {}", self.next, event.seq),
                ));
                return Flow::Stop;
            }
            if self.whole && self.checked == 1 && event.seq.get() != self.first {
                self.misnamed = Some((
                    1,
                    format!(
                        "the segment is named {} but starts with {}",
                        self.first, event.seq
                    ),
                ));
            }
            *self.next = self.next.next();
        }
        if self.misnamed.is_some() {
            return Flow::Go;
        }
        let flow = sink(Piece::Events(events));
        self.stopped = matches!(flow, Flow::Stop);
        flow
    }
}

/// 一窗窗读 `lines`（整行，末尾是 `\n`）：一窗切在最后一个 `\n` 后面，余下的半行挪到下一窗开头；一行比窗口还长的，窗口照
/// 这一行撑大。每窗几个线程一起解（[`parse::stream`]，同以前整段时），解好的块照先后交给 `check` 查、交出去，交完这一窗的
/// 原文就放掉。交回读不成的那一行（从 `lines` 的开头数）和为什么；序号接不上、叫停了的，记在 `check` 里，不读下去。
///
/// 一窗读得完的（几乎所有会话）就在这个线程里读、解，同以前整段读。比一窗大的，另一个线程一窗窗读、解，解好的块排着交过来
/// （最多排 [`QUEUE`] 块）：这一窗还没查完、交完，下一窗已经在读、在解，窗口边上不用等。查、交都在这个线程里，照先后。
fn read_windows(
    file: &mut File,
    lines: std::ops::Range<u64>,
    (window, how): (usize, Parse<'_>),
    check: &mut Check<'_>,
    sink: &mut dyn FnMut(Piece) -> Flow,
) -> io::Result<Option<(usize, String)>> {
    let total = lines.end - lines.start;
    if total == 0 {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(lines.start))?;
    let window = window.max(1);
    if let Ok(size) = usize::try_from(total)
        && size <= window
    {
        let mut buffer = vec![0; size];
        file.read_exact(&mut buffer)?;
        return Ok(parse::stream(&buffer, how, |events| {
            check.block(events, sink)
        }));
    }
    let (sender, receiver) = mpsc::sync_channel(QUEUE);
    std::thread::scope(|scope| {
        let reader = scope.spawn(move || read_ahead(file, total, (window, how), &sender));
        let mut bad = None;
        for read in &receiver {
            match read {
                Ahead::Events(events) => {
                    if matches!(check.block(events, sink), Flow::Stop) {
                        break;
                    }
                }
                Ahead::Broken(number, why) => {
                    bad = Some((number, why));
                    break;
                }
            }
        }
        // 不收了：读的线程下一次交就知道，不读下去。
        drop(receiver);
        let read = reader
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        // 照先后，查出来的错在读的线程后来读不了之前。
        if bad.is_some() || check.wrong.is_some() || check.stopped {
            return Ok(bad);
        }
        read.map(|()| None)
    })
}

/// 比一窗大的段，读、解的线程和查、交的线程之间最多排几块（施工 V-37 中）。
const QUEUE: usize = 8;

/// 读、解的线程交过来的，照先后。
enum Ahead {
    /// 一块事件。
    Events(Vec<Event>),
    /// 读不成的那一行（从读起的地方数）和为什么：后面的不读。
    Broken(usize, String),
}

/// 读、解的线程：`left` 字节一窗窗读、每窗几个线程一起解，解好的块照先后排给 `sender`，读不成的那一行排在最后。那一头不收了
/// 的不读下去。
fn read_ahead(
    file: &mut File,
    mut left: u64,
    (window, how): (usize, Parse<'_>),
    sender: &SyncSender<Ahead>,
) -> io::Result<()> {
    let mut buffer = vec![0; window];
    // 上一窗余下的半行，在 `buffer` 开头。
    let mut held = 0;
    // 前面几窗的行数（都读成了，才走到下一窗）。
    let mut before = 0;
    while left > 0 {
        let (cut, filled) = next_window(file, &mut buffer, held, &mut left, window)?;
        let mut lines = 0;
        let mut gone = false;
        let bad = parse::stream(&buffer[..cut], how, |events| {
            lines += events.len();
            if sender.send(Ahead::Events(events)).is_ok() {
                Flow::Go
            } else {
                gone = true;
                Flow::Stop
            }
        });
        if gone {
            return Ok(());
        }
        if let Some((number, why)) = bad {
            // 块里的行号从这一窗开头数：接上前面几窗的行数。那一头不收了的，不用交了。
            drop(sender.send(Ahead::Broken(before + number, why)));
            return Ok(());
        }
        before += lines;
        buffer.copy_within(cut..filled, 0);
        held = filled - cut;
    }
    Ok(())
}

/// 读下一窗：接在 `buffer` 开头余下的 `held` 个字节（上一窗的半行，比一窗短）后面读满一窗，切在最后一个 `\n` 后面；一行比
/// 窗口还长的，窗口照这一行撑大。交回切在哪、读到了哪。
fn next_window(
    file: &mut File,
    buffer: &mut Vec<u8>,
    held: usize,
    left: &mut u64,
    window: usize,
) -> io::Result<(usize, usize)> {
    let mut start = held;
    let mut target = window;
    loop {
        let more = (target - start).min(usize::try_from(*left).unwrap_or(usize::MAX));
        if more == 0 {
            // 找到的整行末尾那里不再是 `\n`：读的时候这一段被改过（日志只追加，不该有）。
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the segment changed while it was read",
            ));
        }
        if buffer.len() < start + more {
            buffer.resize(start + more, 0);
        }
        file.read_exact(&mut buffer[start..start + more])?;
        *left -= more as u64;
        let filled = start + more;
        if let Some(at) = memchr::memrchr(b'\n', &buffer[start..filled]) {
            return Ok((start + at + 1, filled));
        }
        start = filled;
        target = filled + window;
    }
}

/// 第 `at` 个字节是不是 `\n`：文件没有那么长的不是。
fn newline_at(file: &mut File, at: u64, len: u64) -> io::Result<bool> {
    if at >= len {
        return Ok(false);
    }
    file.seek(SeekFrom::Start(at))?;
    let mut byte = [0];
    Ok(fill(file, &mut byte)? == 1 && byte[0] == b'\n')
}

/// `from..len` 里整行的末尾：最后一个 `\n` 后面，没有的是 `from`。从后往前一块块找，不整段读进来。读的时候文件变短了的
/// （载入这个会话的一方刚截掉了末尾的半行），照变短以后的末尾接着找。
fn whole_lines_end(file: &mut File, from: u64, len: u64) -> io::Result<u64> {
    let mut block = vec![0; usize::try_from(len - from).map_or(SCAN, |size| size.min(SCAN))];
    let mut at = len;
    while at > from {
        let start = at.saturating_sub(SCAN as u64).max(from);
        let size = usize::try_from(at - start).unwrap_or(SCAN);
        file.seek(SeekFrom::Start(start))?;
        let got = fill(file, &mut block[..size])?;
        if let Some(newline) = memchr::memrchr(b'\n', &block[..got]) {
            return Ok(start + newline as u64 + 1);
        }
        at = if got < size {
            start + got as u64
        } else {
            start
        };
    }
    Ok(from)
}

/// `from..end` 里有几个 `\n`：一块块流过去数，不留。
fn count_lines(file: &mut File, from: u64, end: u64) -> io::Result<usize> {
    file.seek(SeekFrom::Start(from))?;
    let mut block = vec![0; usize::try_from(end - from).map_or(SCAN, |size| size.min(SCAN))];
    let mut left = end - from;
    let mut lines = 0;
    while left > 0 {
        let size = usize::try_from(left).map_or(block.len(), |left| left.min(block.len()));
        file.read_exact(&mut block[..size])?;
        lines += memchr::memchr_iter(b'\n', &block[..size]).count();
        left -= size as u64;
    }
    Ok(lines)
}

/// 读满 `buffer`，读到文件末尾为止：交回读到了几个字节。
pub(super) fn fill(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut got = 0;
    while got < buffer.len() {
        match file.read(&mut buffer[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(got)
}

/// 截到 `len` 字节，再同步。
fn truncate(path: &Path, len: u64) -> io::Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(len)?;
    file.sync_all()
}

#[cfg(test)]
mod tests;
