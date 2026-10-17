//! 字节级 BPE（`recall.md` 第四条第 5 款，施工 R-5 六补）：照 Hugging Face `tokenizers` 0.23.3 训练的小 BPE（`fixtures/tiny-bpe/`，
//! `fixtures/make_bpe.py` 造、`tools/bpe_files.py` 转）上照它的结果逐个比（`fixtures/bpe-tokens.json`）；截断、字里的特殊词、
//! 词表和合并坏了另外测。真的 granite 词表（`#[ignore]`）照 `MIYU_EMBED_BPE_DIR` 找转出的两个文件，和
//! `fixtures/granite-tokens.json` 比（`MIYU_EMBED_BPE_CASES` 指到另一份同样格式的就比那一份）。

use std::path::Path;

use miyu_embed::bpe::{FilesError, Tokenizer};
use serde::Deserialize;

use crate::support::{fixtures, read};

#[derive(Deserialize)]
struct Case {
    text: String,
    ids: Vec<i64>,
    /// 截到几个（不写的不截）：仓库外那一份是照 512 截的。
    #[serde(default)]
    limit: Option<usize>,
}

const START: &str = "<|startoftext|>";
const END: &str = "<|return|>";

/// 放得下这些句子的上限：不截断。
const ALL: usize = 1 << 20;

fn load(dir: &Path) -> Tokenizer {
    let text = |name: &str| {
        std::fs::read_to_string(dir.join(name))
            .unwrap_or_else(|error| panic!("读不了 {name}：{error}"))
    };
    Tokenizer::new(&text("vocab.txt"), &text("merges.txt"), START, END).expect("读得出")
}

fn tiny() -> Tokenizer {
    load(&fixtures().join("tiny-bpe"))
}

fn cases(name: &str) -> Vec<Case> {
    serde_json::from_str(&read(name)).expect("合写法")
}

fn matches_all(tokenizer: &Tokenizer, cases: &[Case]) {
    for case in cases {
        let limit = case.limit.unwrap_or(ALL);
        assert_eq!(
            tokenizer.encode(&case.text, limit),
            case.ids,
            "{:?}",
            case.text
        );
    }
}

#[test]
fn every_sentence_matches_the_reference() {
    let cases = cases("bpe-tokens.json");
    assert!(cases.len() > 60, "{}", cases.len());
    matches_all(&tiny(), &cases);
}

#[test]
fn a_cut_keeps_the_end_and_equals_the_head_of_the_whole() {
    let tokenizer = tiny();
    for case in cases("bpe-tokens.json") {
        let whole = &case.ids;
        for limit in 2..14 {
            let got = tokenizer.encode(&case.text, limit);
            let want: Vec<i64> = if whole.len() <= limit {
                whole.clone()
            } else {
                whole[..limit - 1]
                    .iter()
                    .chain(whole.last())
                    .copied()
                    .collect()
            };
            assert_eq!(got, want, "{:?} 截到 {limit}", case.text);
        }
    }
    let ids = tokenizer.encode("x", 1);
    assert_eq!(ids.len(), 2, "比 2 小的照 2 算：{ids:?}");
}

#[test]
fn a_special_word_in_the_text_is_plain_text() {
    let tokenizer = tiny();
    let ids = tokenizer.encode("a<|return|>b<|startoftext|>", ALL);
    let (start, end) = (ids[0], ids[ids.len() - 1]);
    assert_eq!(ids.iter().filter(|id| **id == start).count(), 1, "{ids:?}");
    assert_eq!(ids.iter().filter(|id| **id == end).count(), 1, "{ids:?}");
}

/// 小 BPE 的两份原文。
fn tiny_files() -> (String, String) {
    (read("tiny-bpe/vocab.txt"), read("tiny-bpe/merges.txt"))
}

fn refused(vocab: &str, merges: &str) -> FilesError {
    Tokenizer::new(vocab, merges, START, END).expect_err("要拒")
}

#[test]
fn bad_files_are_refused_and_said() {
    let (vocab, merges) = tiny_files();
    let first = vocab.lines().next().expect("有词").to_string();
    let wrong = [
        (
            refused(&format!("{vocab}{first}\n"), &merges),
            format!("vocab has {first} twice"),
        ),
        (
            refused(&vocab.replace(END, "<|gone|>"), &merges),
            format!("vocab has no {END}"),
        ),
        (
            refused(&vocab, &format!("{merges}nospace\n")),
            format!(
                "merges line {} is not two tokens",
                merges.lines().count() + 1
            ),
        ),
        (
            refused(&vocab, "Ġ zzzz\n"),
            "merges line 1: zzzz is not in vocab".to_string(),
        ),
        (
            refused(&vocab, "x y\n"),
            "merges line 1: xy is not in vocab".to_string(),
        ),
        (
            refused(
                &vocab,
                &format!(
                    "{}\n{}\n",
                    merges.lines().next().expect("有"),
                    merges.lines().next().expect("有")
                ),
            ),
            "merges line 2 repeats an earlier pair".to_string(),
        ),
    ];
    for (error, said) in wrong {
        assert_eq!(error.to_string(), said);
    }
    assert_eq!(
        refused(&vocab.replace(START, "<|gone|>"), &merges),
        FilesError::Missing(START.to_string())
    );
}

/// 词表里没有的字节丢掉，照 `tokenizers`（granite 的词表没有大部分控制字符）。
#[test]
fn a_byte_not_in_the_vocab_is_dropped() {
    let (vocab, merges) = tiny_files();
    // 字节 0x01 换成的字是 U+0101：去掉它那一行（换成一个用不到的字，别的编号不动），合并里没有它。
    assert!(vocab.contains("\n\u{101}\n") && !merges.contains('\u{101}'));
    let tokenizer = Tokenizer::new(
        &vocab.replacen("\n\u{101}\n", "\n\u{4e00}\n", 1),
        &merges,
        START,
        END,
    )
    .expect("缺字节的也读得出");
    assert_eq!(tokenizer.encode("a\u{1}", ALL), tokenizer.encode("a", ALL));
    assert_eq!(
        tokenizer.encode("\u{1}\u{1}", ALL),
        tokenizer.encode("", ALL)
    );
    assert_ne!(
        tiny().encode("a\u{1}", ALL),
        tiny().encode("a", ALL),
        "词表里有的不丢"
    );
}

#[test]
fn ids_are_line_numbers_and_windows_line_ends_are_read() {
    let (vocab, merges) = tiny_files();
    let crlf = Tokenizer::new(
        &vocab.replace('\n', "\r\n"),
        &merges.replace('\n', "\r\n"),
        START,
        END,
    )
    .expect("Windows 的换行也认");
    matches_all(&crlf, &cases("bpe-tokens.json"));
    let start = vocab.lines().position(|word| word == START).expect("有");
    assert_eq!(
        tiny().encode("", ALL),
        [start as i64, start as i64 + 2],
        "行号就是编号：start、endoftext、return"
    );
}

#[test]
#[ignore = "要真的 granite 词表：MIYU_EMBED_BPE_DIR（tools/bpe_files.py 转出的目录）"]
fn the_granite_vocab_agrees_with_the_reference() {
    let dir = std::env::var_os("MIYU_EMBED_BPE_DIR").expect("设了 MIYU_EMBED_BPE_DIR");
    let tokenizer = load(Path::new(&dir));
    let cases: Vec<Case> = match std::env::var_os("MIYU_EMBED_BPE_CASES") {
        Some(path) => {
            serde_json::from_str(&std::fs::read_to_string(path).expect("读得了")).expect("合写法")
        }
        None => cases("granite-tokens.json"),
    };
    matches_all(&tokenizer, &cases);
    println!("{} 句一字不差", cases.len());
}
