//! WordPiece 分词，照 `miyu-embed` 的那一份（BERT 的切法）。测试用：Unicode 的控制、格式字符和标点不查类别表，照常见的
//! 几段认，测试里的句子切得一样就够了。

use std::collections::BTreeMap;

/// 一个词超过这么多个字整个算 `[UNK]`。
const LONGEST_WORD: usize = 100;

/// 一份词表照 WordPiece 切。
pub struct Tokenizer {
    vocab: BTreeMap<String, i64>,
    cls: i64,
    sep: i64,
    unk: i64,
}

impl Tokenizer {
    /// 照词表的原文造：一行一个词，行号就是编号。
    ///
    /// # Errors
    ///
    /// 没有 `[CLS]`、`[SEP]`、`[UNK]`，或者一个词写了两行。
    pub fn new(vocab: &str) -> Result<Tokenizer, String> {
        let mut words = BTreeMap::new();
        for (id, word) in (0_i64..).zip(vocab.lines()) {
            if words.insert(word.to_string(), id).is_some() {
                return Err(format!("vocab has {word} twice"));
            }
        }
        let special = |word: &str| {
            words
                .get(word)
                .copied()
                .ok_or_else(|| format!("vocab has no {word}"))
        };
        Ok(Tokenizer {
            cls: special("[CLS]")?,
            sep: special("[SEP]")?,
            unk: special("[UNK]")?,
            vocab: words,
        })
    }

    /// 切成 `[CLS]`、一个个词、`[SEP]`；超过 `limit` 个的截掉后面的，留住 `[SEP]`。
    pub fn encode(&self, text: &str, limit: usize) -> Vec<i64> {
        let mut ids = vec![self.cls];
        for word in words(text) {
            self.pieces(&word, &mut ids);
        }
        ids.truncate(limit.max(2) - 1);
        ids.push(self.sep);
        ids
    }

    /// 从头起每次取最长的、在词表里的一段（不是开头的带 `##`），有一段取不出来整个词算 `[UNK]`。
    fn pieces(&self, word: &str, ids: &mut Vec<i64>) {
        if word.chars().count() > LONGEST_WORD {
            ids.push(self.unk);
            return;
        }
        let mut found = Vec::new();
        let mut start = 0;
        while start < word.len() {
            let piece = word[start..]
                .char_indices()
                .map(|(at, c)| start + at + c.len_utf8())
                .rev()
                .find_map(|end| {
                    let piece = &word[start..end];
                    let key = if start == 0 {
                        piece.to_string()
                    } else {
                        format!("##{piece}")
                    };
                    self.vocab.get(&key).map(|id| (*id, end))
                });
            let Some((id, end)) = piece else {
                ids.push(self.unk);
                return;
            };
            found.push(id);
            start = end;
        }
        ids.extend(found);
    }
}

/// 去掉控制字符，空白处断开，汉字、标点一个一个成词。
fn words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c == '\u{fffd}' || control(c) {
            continue;
        }
        if c.is_whitespace() || chinese(c) || punctuation(c) {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            if !c.is_whitespace() {
                words.push(c.to_string());
            }
            continue;
        }
        word.push(c);
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// 控制字符（制表、换行、回车算空白）和常见的格式字符（零宽、BOM、软连字符）。
fn control(c: char) -> bool {
    (c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
        || matches!(c, '\u{00ad}' | '\u{200b}'..='\u{200f}' | '\u{2060}'..='\u{2064}' | '\u{feff}')
}

/// ASCII 的标点，和通用标点、CJK 标点、全角标点那几段。
fn punctuation(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            c,
            '\u{2010}'..='\u{2027}'
                | '\u{2030}'..='\u{205e}'
                | '\u{3001}'..='\u{3003}'
                | '\u{3008}'..='\u{3011}'
                | '\u{3014}'..='\u{301f}'
                | '\u{ff01}'..='\u{ff0f}'
                | '\u{ff1a}'..='\u{ff20}'
                | '\u{ff3b}'..='\u{ff40}'
                | '\u{ff5b}'..='\u{ff65}'
        )
}

/// 汉字：CJK 统一表意文字那几段。
fn chinese(c: char) -> bool {
    matches!(
        u32::from(c),
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B920..=0x2CEAF
            | 0xF900..=0xFAFF
            | 0x2F800..=0x2FA1F
    )
}
