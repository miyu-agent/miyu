//! 只记不照原样搬来的字的看法（施工 ED-3），和原来每个字节记一对来处的那一张表对拍：随机造文件（CRLF、单个 `\r`、行尾空白、
//! 行首缩进不同、BOM、多字节的字、宽松那一层认的弯引号、全角、省略号、连字，重复的行），三层看法的字一样，每个字的边界换回
//! 原文的位置也逐个一样；`find` 只经这两样用看法，所以找出来的也一样。另外看表有多大：精确的借原文、一项都不记。

use super::{Level, View, fold};

/// 原来的那一张表（施工 4-6 中到施工 ED-3 前的 `View::of`，原样）：看法的字，和它每个字节来自原文的哪一段。
fn table(text: &str, level: Level) -> (String, Vec<(usize, usize)>) {
    let mut out = (String::new(), Vec::new());
    let push = |out: &mut (String, Vec<(usize, usize)>), c: char, span: (usize, usize)| {
        out.0.push(c);
        out.1.extend(std::iter::repeat_n(span, c.len_utf8()));
    };
    let mut blanks: Vec<(char, (usize, usize))> = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        let mut end = at + c.len_utf8();
        if c == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
            if let Some((next, _)) = chars.next() {
                end = next + 1;
            }
            blanks.clear();
            push(&mut out, '\n', (at, end));
            continue;
        }
        if c == '\n' {
            blanks.clear();
            push(&mut out, '\n', (at, end));
            continue;
        }
        match level {
            Level::Exact => push(&mut out, c, (at, end)),
            Level::Loose | Level::Indent => {
                for folded in fold(c) {
                    if folded == ' ' || folded == '\t' {
                        let fresh = out.0.is_empty() || out.0.ends_with('\n');
                        if !(level == Level::Indent && fresh) {
                            blanks.push((folded, (at, end)));
                        }
                        continue;
                    }
                    for (blank, span) in blanks.drain(..) {
                        push(&mut out, blank, span);
                    }
                    push(&mut out, folded, (at, end));
                }
            }
        }
    }
    out
}

/// 不加依赖的伪随机数（xorshift64*），种子固定：红了照种子重现。
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) % n as u64) as usize
    }

    fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
        from[self.below(from.len())]
    }
}

const WORDS: &[&str] = &[
    "fn",
    "let",
    "x",
    "中文",
    "é",
    "😀",
    "\u{201C}hi\u{201D}",
    "it\u{2019}s",
    "a\u{00A0}b",
    "\u{FF21}\u{FF22}",
    "wait\u{2026}",
    "\u{FB01}le",
    "a\u{2014}b",
    "\u{2122}",
    "x\u{00B2}",
    "(",
    "{",
    ";",
    "\"q\"",
    "-",
    "...",
];
const LEADS: &[&str] = &["", "", "    ", "\t", "  ", " \t", "\u{3000}"];
const GAPS: &[&str] = &[" ", " ", "  ", "\t", "\u{2003}"];
const TAILS: &[&str] = &["", "", "", " ", "  ", "\t", "\u{3000}", "\u{00A0}"];
const BREAKS: &[&str] = &["\n", "\n", "\n", "\r\n", "\r\n", "\r", ""];

/// 随机造一份文件。
fn file(rng: &mut Rng) -> String {
    let mut out = String::new();
    if rng.below(8) == 0 {
        out.push('\u{FEFF}');
    }
    let mut made: Vec<String> = Vec::new();
    for _ in 0..rng.below(12) {
        if !made.is_empty() && rng.below(4) == 0 {
            let again = made[rng.below(made.len())].clone();
            out.push_str(&again);
            continue;
        }
        let mut line = rng.pick(LEADS).to_string();
        for i in 0..rng.below(6) {
            if i > 0 {
                line.push_str(rng.pick(GAPS));
            }
            line.push_str(rng.pick(WORDS));
        }
        line.push_str(rng.pick(TAILS));
        line.push_str(rng.pick(BREAKS));
        out.push_str(&line);
        made.push(line);
    }
    out
}

/// `count` 份随机文件，三层看法都和原来的表对拍。
fn same_as_the_table(seed: u64, count: usize) {
    let mut rng = Rng(seed);
    for _ in 0..count {
        let text = file(&mut rng);
        for level in [Level::Exact, Level::Loose, Level::Indent] {
            let view = View::of(&text, level);
            let (was, spans) = table(&text, level);
            assert_eq!(view.text.as_ref(), was, "{text:?} {level:?}");
            let bounds: Vec<usize> = (0..=was.len())
                .filter(|i| was.is_char_boundary(*i))
                .collect();
            for (i, a) in bounds.iter().enumerate() {
                if *a < was.len() {
                    assert_eq!(view.start_of(*a), spans[*a].0, "{text:?} {level:?} 起 {a}");
                }
                for b in &bounds[i + 1..] {
                    assert_eq!(
                        view.original(*a..*b),
                        spans[*a].0..spans[*b - 1].1,
                        "{text:?} {level:?} {a}..{b}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_view_maps_back_like_the_old_table() {
    same_as_the_table(0xED03_0001, 400);
}

/// 长跑（CI 的 randomized 那一项）：同上，种子多。
#[test]
#[ignore = "长跑：CI 的 randomized 那一项跑"]
fn every_view_maps_back_like_the_old_table_for_longer() {
    same_as_the_table(0xED03_0002, 20_000);
}

#[test]
fn a_view_records_only_what_was_not_copied() {
    let line = "let value = 1;\n";
    let text = line.repeat(10_000);
    let exact = View::of(&text, Level::Exact);
    assert!(
        matches!(exact.text, std::borrow::Cow::Borrowed(_)),
        "精确的借原文"
    );
    assert!(exact.marks.is_empty());
    // 宽松的：照原样搬来的一项不记。
    assert!(View::of(&text, Level::Loose).marks.is_empty());
    // 有 CRLF 的：一个换行一项；行尾有空白的：去掉以后的那个换行一项。
    let crlf = line.replace('\n', "\r\n").repeat(100);
    assert_eq!(View::of(&crlf, Level::Exact).marks.len(), 100);
    let trailing = "let value = 1;  \n".repeat(100);
    assert_eq!(View::of(&trailing, Level::Loose).marks.len(), 100);
}
