//! 把一条命令拆成一个个简单命令（施工 5-17）：认得 `&&`、`||`、`;`、`|`、`|&`、`&`、换行、括号，单双引号、反斜杠，
//! `$( )`、反引号、`<( )`、`>( )` 里面的另拆一遍，heredoc 的正文不拆（施工 AS-9，`heredoc.rs`）。不求和 shell 一模一样：只求
//! 认得出边界、拿到每个词的原样。
//!
//! 一个词里有没在单引号里的 `$`、反引号的，记成「说不清」（[`Word::unresolved`]）：值要到运行时才知道。

mod heredoc;

/// 一个词：引号、反斜杠去掉以后的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Word {
    /// 去掉引号以后的字。
    pub(crate) text: String,
    /// 里面有变量、命令替换，值要到运行时才知道。
    pub(crate) unresolved: bool,
}

/// 一个简单命令：几个词，加上它往哪里写（`>`、`>>` 后面的那个）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Simple {
    pub(crate) words: Vec<Word>,
    pub(crate) writes: Vec<Word>,
}

/// 拆出来的：几条管道（一条管道是几个接在一起的简单命令），加上替换里面的那几段原文。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub(crate) pipelines: Vec<Vec<Simple>>,
    /// `$( )`、反引号、`<( )`、`>( )` 里面的原文：另拆一遍。
    pub(crate) inner: Vec<String>,
}

/// 拆 `text`。
pub(crate) fn parse(text: &str) -> Parsed {
    let mut lexer = Lexer::new(text);
    lexer.run();
    lexer.parsed
}

struct Lexer {
    chars: Vec<char>,
    at: usize,
    parsed: Parsed,
    pipeline: Vec<Simple>,
    simple: Simple,
    word: Option<Word>,
    /// 下一个词是重定向到的地方。
    redirect: Option<Redirect>,
    /// 这一行完了要读正文的 heredoc（施工 AS-9）。
    heredocs: Vec<heredoc::Heredoc>,
}

/// 重定向往哪边。
#[derive(Clone, Copy)]
enum Redirect {
    /// 写到那里（`>`、`>>`）：记下来。
    Write,
    /// 从那里读（`<`、`<<`）：那个词不是参数，丢掉。
    Read,
}

impl Lexer {
    fn new(text: &str) -> Lexer {
        Lexer {
            chars: text.chars().collect(),
            at: 0,
            parsed: Parsed::default(),
            pipeline: Vec::new(),
            simple: Simple::default(),
            word: None,
            redirect: None,
            heredocs: Vec::new(),
        }
    }

    fn run(&mut self) {
        while let Some(c) = self.peek(0) {
            match c {
                '\\' => {
                    self.at += 1;
                    if let Some(next) = self.peek(0) {
                        self.at += 1;
                        if next != '\n' {
                            self.push(next);
                        }
                    }
                }
                '\'' => self.single(),
                '"' => self.double(),
                '`' => {
                    let inner = self.until_backtick();
                    self.substitution(inner);
                }
                '$' if self.peek(1) == Some('(') => {
                    self.at += 2;
                    let inner = self.until_close();
                    self.substitution(inner);
                }
                '$' => {
                    self.at += 1;
                    self.mark('$');
                }
                '<' | '>' if self.peek(1) == Some('(') => {
                    self.at += 2;
                    let inner = self.until_close();
                    self.substitution(inner);
                }
                '>' => {
                    self.descriptor();
                    self.at += 1;
                    while matches!(self.peek(0), Some('>' | '|' | '&')) {
                        self.at += 1;
                    }
                    self.redirect = Some(Redirect::Write);
                }
                // heredoc（`<<`、`<<-`，不是 `<<<`）：结束符不是参数，正文到下一行再读。
                '<' if self.peek(1) == Some('<') && self.peek(2) != Some('<') => {
                    self.descriptor();
                    self.at += 2;
                    self.heredoc_start();
                }
                '<' => {
                    self.descriptor();
                    self.at += 1;
                    while matches!(self.peek(0), Some('<' | '&')) {
                        self.at += 1;
                    }
                    self.redirect = Some(Redirect::Read);
                }
                '|' => {
                    self.at += 1;
                    if self.peek(0) == Some('|') {
                        self.at += 1;
                        self.end_pipeline();
                    } else {
                        if self.peek(0) == Some('&') {
                            self.at += 1;
                        }
                        self.end_simple();
                    }
                }
                // `&&` 是两个 `&`：第二个收的是空的。
                '&' => {
                    self.at += 1;
                    self.end_pipeline();
                }
                ';' | '\n' | '(' | ')' | '{' | '}'
                    if self.word.is_none() || matches!(c, ';' | '\n' | '(' | ')') =>
                {
                    self.at += 1;
                    self.end_pipeline();
                    if c == '\n' {
                        self.heredoc_bodies();
                    }
                }
                '#' if self.word.is_none() => self.skip_comment(),
                c if c.is_whitespace() => {
                    self.at += 1;
                    self.end_word();
                }
                c => {
                    self.at += 1;
                    self.push(c);
                }
            }
        }
        self.end_pipeline();
    }

    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    fn push(&mut self, c: char) {
        self.word
            .get_or_insert_with(|| Word {
                text: String::new(),
                unresolved: false,
            })
            .text
            .push(c);
    }

    /// 记一个要到运行时才知道的字：原样留着，词记成说不清。
    fn mark(&mut self, c: char) {
        self.push(c);
        if let Some(word) = &mut self.word {
            word.unresolved = true;
        }
    }

    fn single(&mut self) {
        self.at += 1;
        self.word.get_or_insert_with(|| Word {
            text: String::new(),
            unresolved: false,
        });
        loop {
            match self.peek(0) {
                None => return,
                Some('\'') => {
                    self.at += 1;
                    return;
                }
                Some(c) => {
                    self.at += 1;
                    self.push(c);
                }
            }
        }
    }

    fn double(&mut self) {
        self.at += 1;
        self.word.get_or_insert_with(|| Word {
            text: String::new(),
            unresolved: false,
        });
        loop {
            match self.peek(0) {
                None => return,
                Some('"') => {
                    self.at += 1;
                    return;
                }
                Some('\\') => {
                    self.at += 1;
                    if let Some(next) = self.peek(0) {
                        self.at += 1;
                        self.push(next);
                    }
                }
                Some('`') => {
                    let inner = self.until_backtick();
                    self.substitution(inner);
                }
                Some('$') if self.peek(1) == Some('(') => {
                    self.at += 2;
                    let inner = self.until_close();
                    self.substitution(inner);
                }
                Some('$') => {
                    self.at += 1;
                    self.mark('$');
                }
                Some(c) => {
                    self.at += 1;
                    self.push(c);
                }
            }
        }
    }

    /// 从开着的反引号起，到配上的那一个：交回里面的原文。
    fn until_backtick(&mut self) -> String {
        self.at += 1;
        let start = self.at;
        while let Some(c) = self.peek(0) {
            self.at += 1;
            if c == '`' {
                return self.chars[start..self.at - 1].iter().collect();
            }
        }
        self.chars[start..].iter().collect()
    }

    /// `(` 已经吃掉了，到配上的 `)`：交回里面的原文。里面的引号照样认，免得引号里的括号算数；heredoc 的正文跳过，正文里的
    /// 撇号不当引号（施工 AS-9）。
    fn until_close(&mut self) -> String {
        let start = self.at;
        let mut depth = 1;
        let mut quote: Option<char> = None;
        while let Some(c) = self.peek(0) {
            self.at += 1;
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), _) => {}
                (None, '\'' | '"') => quote = Some(c),
                (None, '<') if self.peek(0) == Some('<') && self.peek(1) != Some('<') => {
                    self.at += 1;
                    self.heredoc_start();
                }
                (None, '\n') if !self.heredocs.is_empty() => self.heredoc_bodies(),
                (None, '(') => depth += 1,
                (None, ')') => {
                    depth -= 1;
                    if depth == 0 {
                        return self.chars[start..self.at - 1].iter().collect();
                    }
                }
                _ => {}
            }
        }
        self.chars[start..].iter().collect()
    }

    /// 一段替换：里面的另拆，词记成说不清。
    fn substitution(&mut self, inner: String) {
        self.parsed.inner.push(inner);
        self.mark('$');
    }

    fn skip_comment(&mut self) {
        while let Some(c) = self.peek(0) {
            if c == '\n' {
                return;
            }
            self.at += 1;
        }
    }

    /// 紧贴着重定向的纯数字是文件描述符（`2>`），不是参数：丢掉；别的照收尾。
    fn descriptor(&mut self) {
        if self
            .word
            .as_ref()
            .is_some_and(|word| word.text.chars().all(|c| c.is_ascii_digit()))
        {
            self.word = None;
        }
        self.end_word();
    }

    fn end_word(&mut self) {
        if let Some(word) = self.word.take() {
            match self.redirect.take() {
                Some(Redirect::Write) => self.simple.writes.push(word),
                Some(Redirect::Read) => {}
                None => self.simple.words.push(word),
            }
        }
    }

    fn end_simple(&mut self) {
        self.end_word();
        let simple = std::mem::take(&mut self.simple);
        if !simple.words.is_empty() || !simple.writes.is_empty() {
            self.pipeline.push(simple);
        }
    }

    fn end_pipeline(&mut self) {
        self.end_simple();
        let pipeline = std::mem::take(&mut self.pipeline);
        if !pipeline.is_empty() {
            self.parsed.pipelines.push(pipeline);
        }
    }
}

#[cfg(test)]
mod tests;
