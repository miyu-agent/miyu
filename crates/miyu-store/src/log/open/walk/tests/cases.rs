//! 要比的几种日志（施工 V-37 中，`tests.rs` 用）：一行行怎么造，好的、末尾有半行的、坏了的、长行和 `\r\n` 的几批。

/// 一段：名字（这一段第一条的序号）和里面的字节。
pub(super) type Segments = Vec<(u64, Vec<u8>)>;

/// 一行事件的原文：第 `seq` 条，正文 `chars` 个字（中文，一个字三个字节，窗口会切在字的中间）。
pub(super) fn line(seq: u64, chars: usize) -> String {
    let text: String = "日志窗口".chars().cycle().take(chars).collect();
    format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"message.user","by":{{"kind":"person"}},"body":{{"blocks":[{{"type":"text","text":"{text}"}}]}}}}"#
    )
}

/// 一个照 `seed` 定的伪随机数（同一个 `seed` 每次一样）。
pub(super) fn mix(seed: u64) -> u64 {
    let mut x = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(0x6A09_E667);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 32)
}

/// 第 `from` 到第 `to` 条，一行一条，长短不一：大多几个到几十个字，隔一阵一行一两百个字。
pub(super) fn lines(from: u64, to: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    for seq in from..=to {
        let roll = mix(seq);
        let chars = match roll % 40 {
            0 => 100 + (roll >> 8) as usize % 150,
            _ => (roll >> 8) as usize % 40,
        };
        bytes.extend_from_slice(line(seq, chars).as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

/// 一段里第 `k` 行（从 1 数）换成 `with`（不带换行）。
pub(super) fn replace_line(bytes: &[u8], k: usize, with: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (n, one) in bytes.split_inclusive(|byte| *byte == b'\n').enumerate() {
        if n + 1 == k {
            out.extend_from_slice(with);
            out.push(b'\n');
        } else {
            out.extend_from_slice(one);
        }
    }
    out
}

/// 每一行改成 `\r\n` 结尾；隔几行前后再加空格、制表（都算 JSON 的空白）。
pub(super) fn crlf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (n, one) in bytes.split_inclusive(|byte| *byte == b'\n').enumerate() {
        let body = one.strip_suffix(b"\n").unwrap_or(one);
        if n % 3 == 1 {
            out.extend_from_slice(b" \t");
            out.extend_from_slice(body);
            out.extend_from_slice(b" \r\n");
        } else {
            out.extend_from_slice(body);
            out.extend_from_slice(b"\r\n");
        }
    }
    out
}

/// 接上一截没写完的半行。
pub(super) fn with_half(mut bytes: Vec<u8>, half: &str) -> Vec<u8> {
    bytes.extend_from_slice(half.as_bytes());
    bytes
}

/// 没写完的半行。
pub(super) const HALF: &str = r#"{"seq":999,"at":"2026-09-25T07:00"#;

/// 好好的几种：一段、几段、空段、分叉出来的。
pub(super) fn fine() -> Vec<(&'static str, Segments)> {
    vec![
        ("一段", vec![(1, lines(1, 120))]),
        (
            "几段",
            vec![(1, lines(1, 40)), (41, lines(41, 70)), (71, lines(71, 110))],
        ),
        (
            "中间一段是空的",
            vec![(1, lines(1, 30)), (20, Vec::new()), (31, lines(31, 50))],
        ),
        ("最后一段是空的", vec![(1, lines(1, 30)), (31, Vec::new())]),
        ("只有空的第一段", vec![(1, Vec::new())]),
        ("分叉出来的从第 9 条起", vec![(9, lines(9, 50))]),
    ]
}

/// 末尾有半行的几种。
pub(super) fn halves() -> Vec<(&'static str, Segments)> {
    vec![
        ("最后一行半截", vec![(1, with_half(lines(1, 80), HALF))]),
        (
            "最后一段只有半行",
            vec![(1, lines(1, 30)), (31, with_half(Vec::new(), HALF))],
        ),
        (
            "不是最后一段的末尾有半行",
            vec![(1, with_half(lines(1, 30), HALF)), (31, lines(31, 40))],
        ),
        (
            "有坏行、末尾又有半行的不是最后一段",
            vec![
                (
                    1,
                    with_half(replace_line(&lines(1, 30), 10, b"not json"), HALF),
                ),
                (31, lines(31, 40)),
            ],
        ),
        (
            "最后一段有坏行、末尾又有半行",
            vec![
                (1, lines(1, 30)),
                (
                    31,
                    with_half(replace_line(&lines(31, 70), 20, b"not json"), HALF),
                ),
            ],
        ),
    ]
}

/// 坏了的几种：读不成、不是 UTF-8、空行、序号断了、段名不对，和它们碰在一起时先报哪个。
pub(super) fn broken() -> Vec<(&'static str, Segments)> {
    vec![
        (
            "中间一行读不成",
            vec![(1, replace_line(&lines(1, 90), 47, b"{\"seq\":47,"))],
        ),
        (
            "中间一行不是 UTF-8",
            vec![(1, replace_line(&lines(1, 90), 47, b"\xff\xfe"))],
        ),
        (
            "中间一行是空的",
            vec![(1, replace_line(&lines(1, 90), 60, b""))],
        ),
        (
            "第一行读不成",
            vec![(1, replace_line(&lines(1, 30), 1, b"[]"))],
        ),
        (
            "序号断了",
            vec![(1, replace_line(&lines(1, 90), 70, line(72, 20).as_bytes()))],
        ),
        ("段名不对", vec![(1, lines(1, 30)), (35, lines(31, 60))]),
        (
            "段名不对、后面又有坏行",
            vec![
                (1, lines(1, 30)),
                (35, replace_line(&lines(31, 60), 20, b"not json")),
            ],
        ),
        (
            "段名不对、后面序号又断了",
            vec![
                (1, lines(1, 30)),
                (35, replace_line(&lines(31, 60), 20, line(90, 3).as_bytes())),
            ],
        ),
        (
            "下一段第一条就接不上",
            vec![(1, lines(1, 30)), (31, lines(32, 40))],
        ),
    ]
}

/// 长行、`\r\n` 结尾的几种。
pub(super) fn shapes() -> Vec<(&'static str, Segments)> {
    let long = |seq: u64, chars: usize| {
        let mut bytes = line(seq, chars).into_bytes();
        bytes.push(b'\n');
        bytes
    };
    vec![
        (
            "长行",
            vec![(1, {
                let mut bytes = lines(1, 10);
                bytes.extend(long(11, 3000));
                bytes.extend(lines(12, 20));
                bytes.extend(long(21, 1500));
                bytes.extend(long(22, 30));
                bytes
            })],
        ),
        (
            "长行后面一行坏了",
            vec![(1, {
                let mut bytes = lines(1, 5);
                bytes.extend(long(6, 2000));
                bytes.extend(b"not json\n");
                bytes.extend(lines(8, 9));
                bytes
            })],
        ),
        (
            "\\r\\n 结尾",
            vec![(1, crlf(&lines(1, 60))), (61, crlf(&lines(61, 80)))],
        ),
        (
            "\\r\\n 结尾、最后半行停在 \\r 上",
            vec![(1, with_half(crlf(&lines(1, 50)), "{\"seq\":51}\r"))],
        ),
    ]
}
