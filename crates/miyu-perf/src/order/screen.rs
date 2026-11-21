//! 伪终端这一头的几样纯函数（施工 LW-6，`internal/blueprint/perf.md`「排函数」）：像 kitty 那样回答终端界面问终端的几样，
//! 去掉转义序列看屏幕上的字。不碰文件系统、不碰进程，伪终端本身在 `order/terminal.rs`。
//!
//! 为什么要回答：终端界面起来时问光标位置、DA1、前景背景色、16 色、图形协议、字格和窗口大小、键盘协议、同步输出（同核心
//! 仓库 `bench/rivals/term.py`）。没人回答的，它要等 6 秒超时才画输入框，走的也是另外几条路，录到的函数和真用时不一样。

/// 伪终端的大小：行、列。
pub const ROWS: u16 = 45;
/// 伪终端的大小：列。
pub const COLUMNS: u16 = 160;

/// 一个字格的像素大小：高、宽（kitty 照字体给，这里照常见的给一个）。
const CELL: (u32, u32) = (20, 10);

/// 16 色（照 `bench/rivals/term.py`）。
const PALETTE: [&str; 16] = [
    "000000", "cd3131", "0dbc79", "e5e510", "2472c8", "bc3fbc", "11a8cd", "e5e5e5", "666666",
    "f14c4c", "23d18b", "f5f543", "3b8eea", "d670d6", "29b8db", "ffffff",
];

/// 前景色、背景色。
const FOREGROUND: &str = "e5e5e5";
const BACKGROUND: &str = "1a1b26";

/// 回答终端界面这一块输出 `data` 里问终端的几样，交回要写回伪终端的字节；没问的交回空的。
///
/// 照块认，不跨块拼：一个问句被劈在两块之间的，这一次不答（终端界面问的每一句都是一次写出来的，实测没有劈开过）。
pub fn answer(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if contains(data, b"\x1b[6n") {
        out.extend_from_slice(b"\x1b[1;1R");
    }
    for rest in after_all(data, b"\x1b]4;") {
        if let (Some(n), tail) = number(rest)
            && (tail.starts_with(b";?\x07") || tail.starts_with(b";?\x1b\\"))
            && let Some(color) = PALETTE.get(n)
        {
            out.extend_from_slice(format!("\x1b]4;{n};{}\x1b\\", rgb(color)).as_bytes());
        }
    }
    if contains(data, b"\x1b]10;?") {
        out.extend_from_slice(format!("\x1b]10;{}\x1b\\", rgb(FOREGROUND)).as_bytes());
    }
    if contains(data, b"\x1b]11;?") {
        out.extend_from_slice(format!("\x1b]11;{}\x1b\\", rgb(BACKGROUND)).as_bytes());
    }
    // 图形协议：`ESC _ G i=<编号>,…,a=q;… ESC \` 是在问支不支持，照编号答 OK。
    if let Some(rest) = after_all(data, b"\x1b_Gi=").next() {
        let (id, tail) = number(rest);
        let until = tail
            .iter()
            .position(|&byte| byte == 0x1b)
            .unwrap_or(tail.len());
        if let Some(id) = id.filter(|_| contains(&tail[..until], b"a=q")) {
            out.extend_from_slice(format!("\x1b_Gi={id};OK\x1b\\").as_bytes());
        }
    }
    if contains(data, b"\x1b[16t") {
        out.extend_from_slice(format!("\x1b[6;{};{}t", CELL.0, CELL.1).as_bytes());
    }
    if contains(data, b"\x1b[14t") {
        let (height, width) = (u32::from(ROWS) * CELL.0, u32::from(COLUMNS) * CELL.1);
        out.extend_from_slice(format!("\x1b[4;{height};{width}t").as_bytes());
    }
    if contains(data, b"\x1b[18t") {
        out.extend_from_slice(format!("\x1b[8;{ROWS};{COLUMNS}t").as_bytes());
    }
    if contains(data, b"\x1b[5n") {
        out.extend_from_slice(b"\x1b[0n");
    }
    if contains(data, b"\x1b[>q") || contains(data, b"\x1b[>0q") {
        out.extend_from_slice(b"\x1bP>|kitty(0.40.1)\x1b\\");
    }
    if contains(data, b"\x1b[?u") {
        out.extend_from_slice(b"\x1b[?0u");
    }
    // 问某个模式开没开（DECRQM）：`ESC [ ? <模式> $ p`，都答「关着、能开」。
    for rest in after_all(data, b"\x1b[?") {
        if let (Some(mode), tail) = number(rest)
            && tail.starts_with(b"$p")
        {
            out.extend_from_slice(format!("\x1b[?{mode};2$y").as_bytes());
        }
    }
    if contains(data, b"\x1b[c") || contains(data, b"\x1b[0c") {
        out.extend_from_slice(b"\x1b[?62;22c");
    }
    out
}

/// 去掉转义序列：CSI（`ESC [ … 终止字节`）、OSC（`ESC ] … BEL` 或 `ESC ] … ESC \`）、DCS、APC、PM（`ESC P`、`ESC _`、`ESC ^`
/// 到 `ESC \`）、选字符集（`ESC (` 加一个字节）和别的两字节序列。剩下的是画在屏幕上的字和换行这些控制字符。没写完的序列
/// 到末尾就停（下一次从记号以后重新去）。
pub fn strip(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut at = 0;
    while at < data.len() {
        if data[at] != 0x1b {
            out.push(data[at]);
            at += 1;
            continue;
        }
        let Some(&kind) = data.get(at + 1) else {
            break;
        };
        at += 2;
        match kind {
            b'[' => {
                // 参数、中间字节，再一个 0x40..=0x7e 的终止字节。
                while at < data.len() && !(0x40..=0x7e).contains(&data[at]) {
                    at += 1;
                }
                at += 1;
            }
            b']' | b'P' | b'_' | b'^' => {
                while at < data.len() {
                    if data[at] == 0x07 && kind == b']' {
                        at += 1;
                        break;
                    }
                    if data[at] == 0x1b && data.get(at + 1) == Some(&b'\\') {
                        at += 2;
                        break;
                    }
                    at += 1;
                }
            }
            b'(' | b')' => at += 1,
            _ => {}
        }
    }
    out
}

/// 屏幕上的字（去掉转义序列以后）里有没有 `needle`。空格不算：终端界面画一行字时跳过空格、用移光标的序列接着画下一个词
/// （实测 `keep\x1b[22;73Hgoing`），去掉序列以后词和词是挨着的。
pub fn shows(data: &[u8], needle: &str) -> bool {
    let unspaced =
        |bytes: &[u8]| -> Vec<u8> { bytes.iter().copied().filter(|&b| b != b' ').collect() };
    contains(&unspaced(&strip(data)), &unspaced(needle.as_bytes()))
}

/// 去掉转义序列以后的最后 `chars` 个字：等不到的时候附在报错后面，看屏幕上停在哪。
pub fn tail(data: &[u8], chars: usize) -> String {
    let text = String::from_utf8_lossy(&strip(data)).into_owned();
    let count = text.chars().count();
    text.chars().skip(count.saturating_sub(chars)).collect()
}

/// `rgb:rrrr/gggg/bbbb`：每个分量两位写两遍，同 xterm 的回法。
fn rgb(hex: &str) -> String {
    let part = |range: std::ops::Range<usize>| hex.get(range).unwrap_or("00").repeat(2);
    format!("rgb:{}/{}/{}", part(0..2), part(2..4), part(4..6))
}

/// 空的 `needle` 算有（`windows(0)` 会 panic）。
fn contains(data: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || data.windows(needle.len()).any(|window| window == needle)
}

/// `data` 里每一处 `prefix` 后面的那一段。
fn after_all<'a>(data: &'a [u8], prefix: &'a [u8]) -> impl Iterator<Item = &'a [u8]> + 'a {
    (0..data.len())
        .filter(move |&at| data[at..].starts_with(prefix))
        .map(move |at| &data[at + prefix.len()..])
}

/// 开头的十进制数和后面剩下的；开头不是数字的是 `None`。
fn number(data: &[u8]) -> (Option<usize>, &[u8]) {
    let digits = data.iter().take_while(|byte| byte.is_ascii_digit()).count();
    let value = std::str::from_utf8(&data[..digits])
        .ok()
        .and_then(|text| text.parse().ok());
    (value, &data[digits..])
}

#[cfg(test)]
mod tests;
