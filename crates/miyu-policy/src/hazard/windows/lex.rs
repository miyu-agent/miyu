//! 把一条 PowerShell 命令拆成一个个简单命令（施工 WN-1）：认得 `;`、`&&`、`||`、`|`、换行，单引号（`''` 是一个 `'`）、
//! 双引号、反引号转义和续行，`#`、`<# #>` 注释，`,` 隔开的数组；`$( )`、`@( )`、词头的 `( )`、`{ }`（脚本块）里面的另拆
//! 一遍。不求和 PowerShell 一模一样：只求认得出边界、拿到每个词的原样。
//!
//! 一个词里有没在单引号里的 `$` 的，记成「说不清」（[`Word::unresolved`]），变量名照原样留在字里（`$env:USERPROFILE\x`），
//! 判路径时认得的几个照家目录换；`$( )` 这类在字里只留一个 `$`。

use super::super::lex::{Simple, Word};

/// 拆出来的：几条管道，加上替换、括号、脚本块里面的那几段原文（另拆一遍）。
#[derive(Debug, Default)]
pub(super) struct Parsed {
    pub(super) pipelines: Vec<Vec<Simple>>,
    pub(super) inner: Vec<String>,
}

/// 拆 `text`。
pub(super) fn parse(text: &str) -> Parsed {
    let mut lexer = Lexer {
        chars: text.chars().collect(),
        ..Lexer::default()
    };
    lexer.run();
    lexer.parsed
}

#[derive(Default)]
struct Lexer {
    chars: Vec<char>,
    at: usize,
    parsed: Parsed,
    pipeline: Vec<Simple>,
    simple: Simple,
    word: Option<Word>,
    /// 下一个词是重定向写到的地方。
    redirect: bool,
}

impl Lexer {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn next_is(&self, c: char) -> bool {
        self.chars.get(self.at + 1) == Some(&c)
    }

    fn run(&mut self) {
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' | ',' => {
                    self.at += 1;
                    self.end_word();
                }
                '\r' | '\n' | ';' => {
                    self.at += 1;
                    self.end_pipeline();
                }
                '|' | '&' if self.next_is(c) => {
                    self.at += 2;
                    self.end_pipeline();
                }
                '|' => {
                    self.at += 1;
                    self.end_simple();
                }
                // 句首的 `&` 是调用运算符（`& 'C:\x\git.exe' …`），别处的是放到后台（PowerShell 7）。
                '&' => {
                    self.at += 1;
                    if self.word.is_some() || !self.simple.words.is_empty() {
                        self.end_pipeline();
                    }
                }
                '#' if self.word.is_none() => self.skip_line(),
                '<' if self.next_is('#') => self.skip_block_comment(),
                '>' => self.redirection(),
                '\'' => self.single(),
                '"' => self.double(),
                '`' => {
                    self.at += 1;
                    self.escaped();
                }
                '$' | '@' if self.next_is('(') || (c == '@' && self.next_is('{')) => {
                    self.at += 1;
                    self.capture();
                }
                '(' | '{' if self.word.is_none() => self.capture(),
                '$' => self.dollar(),
                // 配不上的右括号：词中间的照原样（`HEAD@{1}`），词外的不要。
                ')' | '}' if self.word.is_none() => self.at += 1,
                c => {
                    self.at += 1;
                    self.word().text.push(c);
                }
            }
        }
        self.end_pipeline();
    }

    fn word(&mut self) -> &mut Word {
        self.word.get_or_insert_with(Word::default)
    }

    fn end_word(&mut self) {
        if let Some(word) = self.word.take() {
            if std::mem::take(&mut self.redirect) {
                self.simple.writes.push(word);
            } else {
                self.simple.words.push(word);
            }
        }
    }

    fn end_simple(&mut self) {
        self.end_word();
        let simple = std::mem::take(&mut self.simple);
        if !simple.words.is_empty() {
            self.pipeline.push(simple);
        }
    }

    fn end_pipeline(&mut self) {
        self.end_simple();
        self.redirect = false;
        let pipeline = std::mem::take(&mut self.pipeline);
        if !pipeline.is_empty() {
            self.parsed.pipelines.push(pipeline);
        }
    }

    /// 反引号后面那个字照原样；反引号紧跟换行的是续行。
    fn escaped(&mut self) {
        match self.peek() {
            Some('\r') if self.next_is('\n') => self.at += 2,
            Some('\n') => self.at += 1,
            Some(c) => {
                self.at += 1;
                self.word().text.push(c);
            }
            None => {}
        }
    }

    fn skip_line(&mut self) {
        while self.peek().is_some_and(|c| c != '\n') {
            self.at += 1;
        }
    }

    fn skip_block_comment(&mut self) {
        self.at += 2;
        while let Some(c) = self.peek() {
            self.at += 1;
            if c == '#' && self.peek() == Some('>') {
                self.at += 1;
                return;
            }
        }
    }

    /// `>`、`>>`、`2>`、`*>`、`2>&1`：前面的流编号不是参数；写到流的没有目标，别的下一个词是写到的地方。
    fn redirection(&mut self) {
        if self
            .word
            .as_ref()
            .is_some_and(|word| word.text == "*" || word.text.chars().all(|c| c.is_ascii_digit()))
        {
            self.word = None;
        }
        self.end_word();
        self.at += 1;
        if self.peek() == Some('>') {
            self.at += 1;
        }
        if self.peek() == Some('&') {
            self.at += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.at += 1;
            }
        } else {
            self.redirect = true;
        }
    }

    /// 单引号：原样到下一个单引号，`''` 是一个 `'`。
    fn single(&mut self) {
        self.at += 1;
        self.word();
        while let Some(c) = self.peek() {
            self.at += 1;
            if c == '\'' {
                if self.peek() == Some('\'') {
                    self.at += 1;
                } else {
                    return;
                }
            }
            self.word().text.push(c);
        }
    }

    /// 双引号：反引号转义，`""` 是一个 `"`，`$( )` 另拆，`$` 说不清。
    fn double(&mut self) {
        self.at += 1;
        self.word();
        while let Some(c) = self.peek() {
            match c {
                '"' if self.next_is('"') => {
                    self.at += 2;
                    self.word().text.push('"');
                }
                '"' => {
                    self.at += 1;
                    return;
                }
                '`' => {
                    self.at += 1;
                    self.escaped();
                }
                '$' if self.next_is('(') => {
                    self.at += 1;
                    self.capture();
                }
                '$' => self.dollar(),
                c => {
                    self.at += 1;
                    self.word().text.push(c);
                }
            }
        }
    }

    /// 变量：`$` 连同名字留在字里，说不清；`${名字}` 整个留下。
    fn dollar(&mut self) {
        self.at += 1;
        let braced = self.peek() == Some('{');
        let word = self.word.get_or_insert_with(Word::default);
        word.text.push('$');
        word.unresolved = true;
        if braced {
            while let Some(&c) = self.chars.get(self.at) {
                self.at += 1;
                word.text.push(c);
                if c == '}' {
                    break;
                }
            }
        }
    }

    /// 从这里的 `(`、`{` 起取到配对的那一个（引号里的不算），里面的原文另拆；词里只留一个 `$`，说不清。没配上的取到结尾。
    fn capture(&mut self) {
        let open = self.peek().unwrap_or('(');
        let close = if open == '{' { '}' } else { ')' };
        self.at += 1;
        let start = self.at;
        let mut depth = 1usize;
        let mut quote: Option<char> = None;
        while let Some(c) = self.peek() {
            self.at += 1;
            match (quote, c) {
                (Some(_), '`') => self.at += 1,
                (Some(q), c) if c == q => quote = None,
                (Some(_), _) => {}
                (None, '\'' | '"') => quote = Some(c),
                (None, '`') => self.at += 1,
                (None, c) if c == open => depth += 1,
                (None, c) if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = if depth == 0 { self.at - 1 } else { self.at };
        let end = end.min(self.chars.len());
        let inner: String = self.chars[start.min(end)..end].iter().collect();
        self.parsed.inner.push(inner);
        let word = self.word();
        word.text.push('$');
        word.unresolved = true;
    }
}
