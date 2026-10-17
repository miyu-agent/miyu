//! 字节级 BPE 分词（施工 R-5 六补，`docs/blueprint/recall.md` 第四条第 5 款）：granite-embedding-97m-multilingual-r2 这一类，
//! GPT-4o 的那一套切法。和 Hugging Face `tokenizers` 照那份 `tokenizer.json`（`Split` 单独切、`ByteLevel` 不加前导空格不再切、
//! `BPE` 的 `ignore_merges`、模板前后各加一个特殊的词）切得一样（`tests/bpe.rs` 照它的结果逐个比）。不读 `tokenizer.json`：
//! 词表、合并各一份一行一条的文本（`tools/bpe_files.py` 转），解析大 JSON 冲高的内存不还。只编码，不解码。
//!
//! 和它不一样的只有两处：
//! - 字里写着特殊的词（`<|return|>` 这种）的，它当成那个词，这里照普通的字切：人说的话不变成控制用的词。词表里的特殊的词都是
//!   标点夹着字母，正则总把它们切开，整段查词表也查不到它们。
//! - 正则里 Unicode 的类别照 `regex` 的表，它照 Oniguruma 的：新分配的字可能切得不一样。

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};

use fancy_regex::Regex;

/// 切段的正则：GPT-4o（o200k）那一条，和 `tools/bpe_files.py` 的 `SPLIT` 一字不差（转换脚本只收这一条）。
pub const SPLIT: &str = concat!(
    r"[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]*[\p{Ll}\p{Lm}\p{Lo}\p{M}]+(?i:'s|'t|'re|'ve|'m|'ll|'d)?",
    r"|[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]+[\p{Ll}\p{Lm}\p{Lo}\p{M}]*(?i:'s|'t|'re|'ve|'m|'ll|'d)?",
    r"|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n/]*|\s*[\r\n]+|\s+(?!\S)|\s+",
);

/// 查词表、合并用的哈希：FxHash 的做法（一次一个字长，乘一个常数）。载入时要查六十来万次，标准库的 SipHash 慢一倍多；
/// 键是模型自己的词表和切出来的一段字，碰撞攻击无从谈起。
#[derive(Debug, Clone, Copy, Default)]
struct Fx(u64);

impl Hasher for Fx {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let mut word = [0; 8];
            word.copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
        for byte in chunks.remainder() {
            self.add(u64::from(*byte));
        }
    }

    fn write_u32(&mut self, n: u32) {
        self.add(u64::from(n));
    }

    fn write_u8(&mut self, n: u8) {
        self.add(u64::from(n));
    }

    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

impl Fx {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

type Map<K, V> = HashMap<K, V, BuildHasherDefault<Fx>>;

/// 一份词表和合并照字节级 BPE 切。
#[derive(Debug, Clone)]
pub struct Tokenizer {
    /// 词和它的编号（行号）。
    vocab: Map<Box<str>, u32>,
    /// 相邻的两个编号能合成哪一个：（先后，合成的编号）。
    merges: Map<(u32, u32), (u32, u32)>,
    /// 每个字节换成的字（GPT-2 的表）。
    chars: [char; 256],
    /// 每个字节那个字的编号；词表里没有的（granite 没有大部分控制字符、`0xF1` 以后的几个）丢掉，照 `tokenizers`（没有
    /// `unk_token`、不退到字节编号时它就丢）。
    bytes: [Option<u32>; 256],
    split: Regex,
    start: u32,
    end: u32,
}

/// 词表、合并读不出来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesError {
    /// 词表里少了清单写的特殊的词。
    Missing(String),
    /// 同一个词写了两行：编号该是哪一行说不清。
    Repeated(String),
    /// 合并的第几行（从 1 数）不是空格隔开的两个词。
    NotAPair(usize),
    /// 合并的第几行里有个词（左、右或合成的）不在词表里。
    Unknown {
        /// 第几行，从 1 数。
        line: usize,
        /// 哪个词。
        word: String,
    },
    /// 合并的第几行和前面的哪一行是同一对。
    Twice(usize),
}

impl fmt::Display for FilesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilesError::Missing(word) => write!(f, "vocab has no {word}"),
            FilesError::Repeated(word) => write!(f, "vocab has {word} twice"),
            FilesError::NotAPair(line) => write!(f, "merges line {line} is not two tokens"),
            FilesError::Unknown { line, word } => {
                write!(f, "merges line {line}: {word} is not in vocab")
            }
            FilesError::Twice(line) => write!(f, "merges line {line} repeats an earlier pair"),
        }
    }
}

impl std::error::Error for FilesError {}

/// GPT-2 的字节到字：看得见的（`!` 到 `~`、`¡` 到 `¬`、`®` 到 `ÿ`）照原样，别的照先后挪到 `U+0100` 以后。
fn byte_chars() -> [char; 256] {
    let mut chars = ['\0'; 256];
    let mut moved = 0;
    for (byte, slot) in (0_u32..).zip(chars.iter_mut()) {
        let visible = matches!(byte, 0x21..=0x7E | 0xA1..=0xAC | 0xAE..=0xFF);
        let code = if visible {
            byte
        } else {
            moved += 1;
            0xFF + moved
        };
        *slot = char::from_u32(code).unwrap_or('\0');
    }
    chars
}

/// 合并时的一格：一个词，连着左右还在的那两格。
#[derive(Debug, Clone, Copy)]
struct Symbol {
    id: u32,
    prev: Option<usize>,
    next: Option<usize>,
    /// 合进左边那一格了。
    gone: bool,
}

impl Tokenizer {
    /// 照词表的原文 `vocab`（一行一个词，第几行从 0 数就是编号）、合并的原文 `merges`（一行一条「左 右」，第几行就是先后）造，
    /// 前后加 `start`、`end`；`\n`、`\r\n` 都认。
    ///
    /// # Errors
    ///
    /// 一个词写了两行；没有 `start`、`end`；合并的一行不是两个词、有词不在词表里、一对写了两行。
    ///
    /// # Panics
    ///
    /// 不会：[`SPLIT`] 是写死的、合写法的正则（每个测试造一份就编一次）。
    pub fn new(vocab: &str, merges: &str, start: &str, end: &str) -> Result<Tokenizer, FilesError> {
        let mut words = Map::default();
        words.reserve(vocab.lines().count());
        for (id, word) in (0_u32..).zip(vocab.lines()) {
            if words.insert(Box::<str>::from(word), id).is_some() {
                return Err(FilesError::Repeated(word.to_string()));
            }
        }
        let find = |word: &str| words.get(word).copied();
        let special = |word: &str| find(word).ok_or_else(|| FilesError::Missing(word.to_string()));
        let (start, end) = (special(start)?, special(end)?);
        let chars = byte_chars();
        let mut bytes = [None; 256];
        for (c, slot) in chars.iter().zip(bytes.iter_mut()) {
            *slot = find(c.encode_utf8(&mut [0; 4]));
        }
        let mut pairs = Map::default();
        pairs.reserve(merges.lines().count());
        let mut joined = String::new();
        for (rank, (at, line)) in (0_u32..).zip(merges.lines().enumerate()) {
            let line_no = at + 1;
            let (left, right) = line.split_once(' ').ok_or(FilesError::NotAPair(line_no))?;
            joined.clear();
            joined.push_str(left);
            joined.push_str(right);
            let known = |word: &str| {
                find(word).ok_or_else(|| FilesError::Unknown {
                    line: line_no,
                    word: word.to_string(),
                })
            };
            let pair = (known(left)?, known(right)?);
            if pairs.insert(pair, (rank, known(&joined)?)).is_some() {
                return Err(FilesError::Twice(line_no));
            }
        }
        words.shrink_to_fit();
        pairs.shrink_to_fit();
        Ok(Tokenizer {
            vocab: words,
            merges: pairs,
            chars,
            bytes,
            // 写死的一条，编得过（`tests/bpe.rs` 造一份就编一次）。
            split: Regex::new(SPLIT).expect("SPLIT 是合写法的正则"),
            start,
            end,
        })
    }

    /// 把 `text` 切成编号：`start`、一段段的词、`end`。超过 `limit` 个的截掉后面的，留住 `end`；够了就不再往后切。`limit`
    /// 至少是 2（清单查过），更小的照 2 算。
    pub fn encode(&self, text: &str, limit: usize) -> Vec<i64> {
        let keep = limit.max(2) - 1;
        let mut ids = vec![self.start];
        let mut last = 0;
        for found in self.split.find_iter(text) {
            if ids.len() >= keep {
                break;
            }
            // 回溯超了 `fancy-regex` 的上限才会出错（这条正则只往前看一个字，碰不到）：剩下的当一段。
            let Ok(found) = found else {
                self.piece(&text[last..], &mut ids);
                break;
            };
            // 这条正则每个字都有一支接得住（字母、数字、空白、别的），段和段之间没有漏下的字，`tokenizers` 单独切的空隙
            // 这里碰不到。
            self.piece(found.as_str(), &mut ids);
            last = found.end();
        }
        ids.truncate(keep);
        ids.push(self.end);
        ids.into_iter().map(i64::from).collect()
    }

    /// 一段：字节照表换成字，整段在词表里的直接用；不然从一个个字节起合（词表里没有的字节丢掉）。
    fn piece(&self, piece: &str, ids: &mut Vec<u32>) {
        let whole: String = piece
            .bytes()
            .map(|byte| self.chars[usize::from(byte)])
            .collect();
        if let Some(id) = self.vocab.get(whole.as_str()) {
            ids.push(*id);
            return;
        }
        let known: Vec<u32> = piece
            .bytes()
            .filter_map(|byte| self.bytes[usize::from(byte)])
            .collect();
        let count = known.len();
        let mut symbols: Vec<Symbol> = known
            .into_iter()
            .enumerate()
            .map(|(at, id)| Symbol {
                id,
                prev: at.checked_sub(1),
                next: (at + 1 < count).then_some(at + 1),
                gone: false,
            })
            .collect();
        self.merge(&mut symbols);
        ids.extend(
            symbols
                .iter()
                .filter(|symbol| !symbol.gone)
                .map(|symbol| symbol.id),
        );
    }

    /// 照 `tokenizers` 的 `Word::merge_all`：每次合先后最靠前的一对，同一对取最左边；合了以后和左右两格新成的对再排进去，
    /// 过时的（左边那格已合走、右边换了）跳过。
    fn merge(&self, symbols: &mut [Symbol]) {
        let mut queue: BinaryHeap<Reverse<(u32, usize, u32)>> = symbols
            .windows(2)
            .enumerate()
            .filter_map(|(at, pair)| {
                let (rank, to) = self.merges.get(&(pair[0].id, pair[1].id))?;
                Some(Reverse((*rank, at, *to)))
            })
            .collect();
        while let Some(Reverse((_, at, to))) = queue.pop() {
            let left = symbols[at];
            let Some(right_at) = left.next else { continue };
            if left.gone {
                continue;
            }
            let right = symbols[right_at];
            if self.merges.get(&(left.id, right.id)).map(|(_, id)| *id) != Some(to) {
                continue;
            }
            symbols[at].id = to;
            symbols[at].next = right.next;
            symbols[right_at].gone = true;
            if let Some(after) = right.next {
                symbols[after].prev = Some(at);
            }
            let current = symbols[at];
            if let Some(before) = current.prev
                && let Some((rank, id)) = self.merges.get(&(symbols[before].id, current.id))
            {
                queue.push(Reverse((*rank, before, *id)));
            }
            if let Some(after) = current.next
                && let Some((rank, id)) = self.merges.get(&(current.id, symbols[after].id))
            {
                queue.push(Reverse((*rank, at, *id)));
            }
        }
    }
}
