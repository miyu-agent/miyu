//! 一批写到一半出错（施工 AUD-5：盘满这类）：段文件截回写之前的长短，下一批接在整行后面，日志中间不留半行。
//! `File` 注入不了失败，这里拿一个替身句柄：照真的段文件写进前一半，再报错；三个平台都一样跑。

use std::fs::File;
use std::io::{self, Write};

use super::*;

/// 写进前一半就报错的句柄：交给它的第一块只写进前一半，后面的一律报错，像写到一半盘满了。
struct Half {
    file: File,
    failed: bool,
}

impl Write for Half {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.failed {
            return Err(io::Error::other("disk full"));
        }
        self.failed = true;
        self.file.write(&buf[..buf.len() / 2])
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// 拼好的一批：照 [`SessionLog::write`] 的写法，一行一条。
fn batch(events: &[Event]) -> Vec<u8> {
    events
        .iter()
        .flat_map(|event| line_of(event).into_bytes())
        .collect()
}

/// 往 `log` 正在写的那一段写一批，写到一半失败；交回段文件那时多长、该多长。
fn tear(log: &SessionLog, events: &[Event]) -> (u64, u64) {
    let mark = log.mark();
    let path = log.dir().join(segment_name_of(mark.segment));
    let mut half = Half {
        file: log.sync_handle().unwrap(),
        failed: false,
    };
    let error = put(
        &mut half,
        &batch(events),
        (log.dir(), mark.segment),
        mark.bytes,
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "disk full", "交回的是写的错");
    (fs::metadata(&path).unwrap().len(), mark.bytes)
}

#[test]
fn a_batch_that_fails_halfway_leaves_no_partial_line() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).unwrap();
    log.append(&said_range(1, 3)).unwrap();
    let (len, want) = tear(&log, &said_range(4, 5));
    assert_eq!(len, want, "写了一半的截掉了");
    // 下一批接在整行后面：关掉再打开，一行不坏、一条不缺。
    log.append(&said_range(4, 5)).unwrap();
    drop(log);
    let (_, events) = SessionLog::open(&dir, SEGMENT_LIMIT).unwrap();
    assert_eq!(events, said_range(1, 5));
}

#[test]
fn a_fresh_segment_that_fails_halfway_is_left_empty() {
    // 刚建好、还空着的一段（换段以后写之前也是这样，长 0）：截回空的，下一批从它的开头写。
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create_at(&dir, SEGMENT_LIMIT, Seq::new(7).unwrap()).unwrap();
    let (len, want) = tear(&log, &said_range(7, 8));
    assert_eq!((len, want), (0, 0), "空的段截回空的");
    log.append(&said_range(7, 8)).unwrap();
    drop(log);
    let (_, events) = SessionLog::open(&dir, SEGMENT_LIMIT).unwrap();
    assert_eq!(events, said_range(7, 8));
}
