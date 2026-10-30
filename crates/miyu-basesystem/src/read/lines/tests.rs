//! 按行读：编码、二进制、分页、行号、截长行、64 KiB 的上限；整份文件的内容哈希（施工 4-6 上）；一行只留开头、UTF-16 边读
//! 边解，读出来的和整份读进来的一样，叫停了不读下去（审计 E-3）。

use std::io::Write;

use super::*;
use crate::common::OUTPUT_BYTES;

/// 一个用完就删的临时文件，内容是 `bytes`。
struct Temp(std::path::PathBuf);

impl Temp {
    fn with(bytes: &[u8]) -> Temp {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("miyu-lines-{}-{n}", std::process::id()));
        std::fs::File::create(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        Temp(path)
    }

    fn page(&self, offset: u64, limit: u64) -> Page {
        read(
            File::open(&self.0).unwrap(),
            offset,
            limit,
            &Stop::default(),
        )
        .unwrap()
        .page
    }

    /// 读一页算出来的整份文件的哈希。
    fn hash(&self, offset: u64, limit: u64) -> Option<ContentHash> {
        read(
            File::open(&self.0).unwrap(),
            offset,
            limit,
            &Stop::default(),
        )
        .unwrap()
        .hash
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

fn lines(text: &str, from: u64, to: u64, total: u64) -> Page {
    Page::Lines {
        text: text.to_string(),
        from,
        to,
        total,
    }
}

#[test]
fn lines_come_with_a_number_and_a_tab() {
    let file = Temp::with(b"one\ntwo\r\nthree");
    assert_eq!(
        file.page(1, 2000),
        lines("1\tone\n2\ttwo\n3\tthree\n", 1, 3, 3)
    );
    assert_eq!(file.page(2, 1), lines("2\ttwo\n", 2, 2, 3));
    assert_eq!(file.page(4, 10), Page::PastEnd { total: 3 });
    // 以换行结尾的，最后一行后面没有空行。
    assert_eq!(Temp::with(b"a\n").page(1, 10), lines("1\ta\n", 1, 1, 1));
}

#[test]
fn empty_binary_and_bom() {
    assert_eq!(Temp::with(b"").page(1, 10), Page::Empty);
    assert_eq!(Temp::with(b"\xEF\xBB\xBF").page(1, 10), Page::Empty);
    assert_eq!(Temp::with(b"\x7FELF\0\0\x01").page(1, 10), Page::Binary);
    assert_eq!(
        Temp::with(b"\xEF\xBB\xBFhi\n").page(1, 10),
        lines("1\thi\n", 1, 1, 1),
        "UTF-8 的 BOM 不显示"
    );
}

#[test]
fn utf16_both_byte_orders() {
    let text = "你好\r\nMiyu\n";
    let little: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let big: Vec<u8> = [0xFE, 0xFF]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_be_bytes))
        .collect();
    let expected = lines("1\t你好\n2\tMiyu\n", 1, 2, 2);
    assert_eq!(Temp::with(&little).page(1, 10), expected);
    assert_eq!(Temp::with(&big).page(1, 10), expected);
    assert_eq!(Temp::with(&[0xFF, 0xFE]).page(1, 10), Page::Empty);
}

#[test]
fn a_long_line_is_cut() {
    let long = "字".repeat(LINE_CHARS + 5);
    let Page::Lines { text, .. } = Temp::with(long.as_bytes()).page(1, 10) else {
        panic!("该读得出来");
    };
    let shown = text.trim_start().trim_start_matches("1\t").trim_end();
    assert_eq!(shown.chars().count(), LINE_CHARS + 1);
    assert!(shown.ends_with('…'));
}

#[test]
fn a_page_stops_at_the_output_limit() {
    let line = "x".repeat(1000);
    let body: String = (0..200).map(|_| format!("{line}\n")).collect();
    let Page::Lines {
        text,
        from,
        to,
        total,
    } = Temp::with(body.as_bytes()).page(1, 2000)
    else {
        panic!("该读得出来");
    };
    assert_eq!((from, total), (1, 200));
    assert!(to < total, "到了 64 KiB 就停：{to}");
    assert!(text.len() <= OUTPUT_BYTES);
    assert_eq!(text.lines().count() as u64, to);
}

#[test]
fn the_hash_is_of_the_whole_file_however_much_is_shown() {
    let mut long = String::new();
    for n in 0..5000 {
        long.push_str(&format!("line {n}\r\n"));
    }
    let cases: [&[u8]; 7] = [
        b"one\ntwo\nthree\n",
        b"\xEF\xBB\xBFwith a bom\n",
        b"\xFF\xFEa\x00\n\x00",
        b"crlf\r\nlines\r\n",
        b"",
        b"no newline at the end",
        long.as_bytes(),
    ];
    for bytes in cases {
        let file = Temp::with(bytes);
        let whole = Some(ContentHash::of(bytes));
        assert_eq!(
            file.hash(1, 2000),
            whole,
            "{:?}",
            &bytes[..bytes.len().min(20)]
        );
        assert_eq!(file.hash(2, 1), whole, "读一段的也是整份的");
        assert_eq!(file.hash(9999, 10), whole, "过了结尾的也是整份的");
    }
    let binary = Temp::with(b"a\x00b");
    assert_eq!(
        binary.hash(1, 2000),
        Some(ContentHash::of(b"a\x00b")),
        "二进制的不读内容，哈希照样是整份的"
    );
    // 比开头认编码读的那一段长的，后面那一截也算进去。
    let mut long = vec![0_u8; 3];
    long.extend(std::iter::repeat_n(b'x', 20_000));
    assert_eq!(Temp::with(&long).hash(1, 1), Some(ContentHash::of(&long)));
}

/// 施工 V-31 以前的 `page`，原样留下：每一行都解成字再挑，页外的也一样。只用来对照。
fn page_before(
    lines: impl Iterator<Item = io::Result<String>>,
    offset: u64,
    limit: u64,
) -> io::Result<Page> {
    let mut text = String::new();
    let mut total = 0;
    let mut to = offset.saturating_sub(1);
    let mut full = false;
    for line in lines {
        let line = line?;
        total += 1;
        if total < offset || full || total >= offset + limit {
            continue;
        }
        let numbered = format!("{total}\t{}\n", cut(&line));
        if text.len() + numbered.len() > OUTPUT_BYTES && total > offset {
            full = true;
            continue;
        }
        text.push_str(&numbered);
        to = total;
    }
    Ok(if total == 0 {
        Page::Empty
    } else if offset > total {
        Page::PastEnd { total }
    } else {
        Page::Lines {
            text,
            from: offset,
            to,
            total,
        }
    })
}

/// 以前的读法（审计 E-3 以前）：整行切出来、整份 UTF-16 读进来再解，每一行都解成字（施工 V-31 以前）。只用来对照，读出来的
/// 要一字不差。
fn whole(bytes: &[u8], offset: u64, limit: u64) -> Page {
    let utf16 = |unit: fn([u8; 2]) -> u16| {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| unit([pair[0], pair[1]]))
            .collect();
        let text = String::from_utf16_lossy(&units);
        let count = if text.is_empty() {
            0
        } else {
            text.split('\n').count() - usize::from(text.ends_with('\n'))
        };
        let lines: Vec<io::Result<String>> = text
            .split('\n')
            .take(count)
            .map(|line| Ok(line.strip_suffix('\r').unwrap_or(line).to_string()))
            .collect();
        page_before(lines.into_iter(), offset, limit).unwrap()
    };
    match bytes {
        [0xFF, 0xFE, ..] => return utf16(u16::from_le_bytes),
        [0xFE, 0xFF, ..] => return utf16(u16::from_be_bytes),
        _ => {}
    }
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let lines = body.split(|byte| *byte == b'\n').map(|line| {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        Ok(String::from_utf8_lossy(line).into_owned())
    });
    // `split` 对以换行结尾的多切一段空的、对空的切出一段空的：`BufRead::split` 都不交。
    let mut lines: Vec<io::Result<String>> = lines.collect();
    if body.is_empty() || body.ends_with(b"\n") {
        lines.pop();
    }
    page_before(lines.into_iter(), offset, limit).unwrap()
}

/// UTF-16 带 BOM 的字节：`little` 是小端。
fn wide(text: &[u16], little: bool) -> Vec<u8> {
    let bom: [u8; 2] = if little { [0xFF, 0xFE] } else { [0xFE, 0xFF] };
    bom.into_iter()
        .chain(text.iter().flat_map(|unit| match little {
            true => unit.to_le_bytes(),
            false => unit.to_be_bytes(),
        }))
        .collect()
}

/// 一行只留开头（审计 E-3）：截线前后的长度、行尾的 `\r` 正好落在截线上、截线落在一个字中间、坏字节、UTF-16 的代理对和
/// 落单的代理、最后多半个字，读出来都和整份读进来的一样。
#[test]
fn keeping_the_head_of_a_line_reads_the_same_as_the_whole() {
    let mut cases: Vec<Vec<u8>> = Vec::new();
    for n in [LINE_CHARS - 1, LINE_CHARS, LINE_CHARS + 1, LINE_CHARS + 2] {
        cases.push("x".repeat(n).into_bytes());
        cases.push(format!("{}\r\nnext\n", "x".repeat(n)).into_bytes());
        cases.push(format!("{}\n", "字".repeat(n)).into_bytes());
        cases.push(format!("{}\r\n", "😀".repeat(n)).into_bytes());
    }
    for n in [
        LINE_BYTES - 2,
        LINE_BYTES - 1,
        LINE_BYTES,
        LINE_BYTES + 1,
        LINE_BYTES + 2,
    ] {
        let mut line = vec![b'a'; n];
        line.push(b'\r');
        line.extend_from_slice(b"\nafter\r");
        cases.push(line);
        // 截线落在一个四字节字的中间、坏字节在截线附近。
        let mut cut = vec![b'b'; n.saturating_sub(2)];
        cut.extend_from_slice("😀".as_bytes());
        cut.extend_from_slice(b"\xFF\xFE tail\n");
        cases.push(cut);
        let mut bad = vec![0xC3; n];
        bad.push(b'\n');
        cases.push(bad);
    }
    cases.push(b"\xEF\xBB\xBF\r\n\n\r".to_vec());
    cases.push(b"\n\n\n".to_vec());
    let mut units: Vec<u16> = "a\r\n".encode_utf16().collect();
    units.extend("😀".repeat(LINE_CHARS + 3).encode_utf16());
    units.extend([0xD800, u16::from(b'x'), 0xDC00, u16::from(b'\n')]);
    units.extend("y".repeat(LINE_CHARS).encode_utf16());
    units.extend("\r\nz".encode_utf16());
    for little in [true, false] {
        cases.push(wide(&units, little));
        let mut odd = wide(&units, little);
        odd.push(0x41);
        cases.push(odd);
        cases.push(wide(
            &"x".repeat(LINE_CHARS + 1)
                .encode_utf16()
                .collect::<Vec<_>>(),
            little,
        ));
    }
    for (n, bytes) in cases.iter().enumerate() {
        for (offset, limit) in [(1, 2000), (2, 1), (3, 5)] {
            assert_eq!(
                Temp::with(bytes).page(offset, limit),
                whole(bytes, offset, limit),
                "第 {n} 份，第 {offset} 行起 {limit} 行"
            );
        }
    }
}

/// 叫停了不往下读：交回 `Interrupted`。
#[test]
fn a_stopped_read_does_not_go_on() {
    let file = Temp::with(b"one\ntwo\n");
    let stop = Stop::default();
    stop.raise();
    let error = read(File::open(&file.0).unwrap(), 1, 10, &stop).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    let wide_file = Temp::with(&wide(&"one\n".encode_utf16().collect::<Vec<_>>(), true));
    let error = read(File::open(&wide_file.0).unwrap(), 1, 10, &stop).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
}

/// 页外的行只数不解（施工 V-31）：一批输入、一批从哪起读几行（页前、页后、正好是最后一行、过了结尾、停在 64 KiB），读出来
/// 的和每一行都解的老读法一字不差，哈希照旧是整份的。跳过的那一段里有长过 [`LINE_BYTES`] 的行、`\r\n`、坏字节、跨过读的
/// 那一块（8 KiB）的行，都不影响后面行号和字。
#[test]
fn skipping_the_lines_off_the_page_reads_the_same_as_before() {
    let mut cases: Vec<Vec<u8>> = Vec::new();
    let mut mixed = Vec::new();
    for n in 0..300_usize {
        match n % 7 {
            0 => mixed.extend_from_slice(b"\r\n"),
            1 => mixed.extend(std::iter::repeat_n(b'l', LINE_BYTES + 37 * n)),
            2 => mixed.extend_from_slice("字😀\u{FFFD}".repeat(n).as_bytes()),
            3 => mixed.extend_from_slice(b"\xFF\xC3 bad\r"),
            4 => mixed.extend(std::iter::repeat_n(b'k', 8 * 1024 - 1)),
            _ => mixed.extend_from_slice(format!("line {n}").as_bytes()),
        }
        mixed.push(b'\n');
    }
    cases.push(mixed.clone());
    mixed.extend_from_slice(b"no newline at the end\r");
    cases.push(mixed.clone());
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(&mixed);
    cases.push(bom);
    // 一行一千多个字节：六十来行就到 64 KiB。
    cases.push(
        (0..500)
            .map(|n| format!("{n:04}{}\n", "w".repeat(1100)))
            .collect::<String>()
            .into_bytes(),
    );
    cases.push(b"\n".repeat(20_000));
    cases.push(b"only".to_vec());
    let text: String = (0..400).map(|n| format!("第 {n} 行\r\n")).collect();
    let units: Vec<u16> = text.encode_utf16().collect();
    cases.push(wide(&units, true));
    cases.push(wide(&units, false));
    for (n, bytes) in cases.iter().enumerate() {
        let file = Temp::with(bytes);
        for (offset, limit) in [
            (1, 2000),
            (1, 1),
            (2, 1),
            (5, 3),
            (40, 70),
            (150, 2000),
            (299, 5),
            (300, 1),
            (301, 1),
            (302, 10),
            (400, 2000),
            (401, 1),
            (499, 2),
            (19_999, 2000),
            (20_001, 1),
            (1_000_000, 2000),
        ] {
            assert_eq!(
                file.page(offset, limit),
                whole(bytes, offset, limit),
                "第 {n} 份，第 {offset} 行起 {limit} 行"
            );
            assert_eq!(
                file.hash(offset, limit),
                Some(ContentHash::of(bytes)),
                "第 {n} 份，第 {offset} 行起 {limit} 行"
            );
        }
    }
}

/// 只跳过、一行都不要的时候叫停了，照样不往下读。
#[test]
fn a_stopped_read_does_not_go_on_while_skipping() {
    let file = Temp::with(&b"skipped\n".repeat(10_000));
    let stop = Stop::default();
    stop.raise();
    let error = read(File::open(&file.0).unwrap(), 1_000_000, 10, &stop).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
}
