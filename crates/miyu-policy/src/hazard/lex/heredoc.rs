//! heredoc（施工 AS-9）：`<<结束符`、`<<-结束符` 的正文从下一行起，到正好是结束符的那一行为止，不是命令、不拆；正文里的撇号
//! （`don't`）不当引号。原来照命令拆，从撇号起一直吞到结尾，后面的删除就认不出了（常见的 `git commit -m "$(cat <<'EOF' …`）。
//! 结束符没加引号的，正文里的 `$( )`、反引号照样会执行：里面的另拆一遍。

use super::Lexer;

/// 等着从下一行起读正文的一个 heredoc。
pub(super) struct Heredoc {
    /// 结束符（引号、反斜杠去掉以后）。
    end: String,
    /// `<<-`：结束符那一行前面的制表符不算。
    strip: bool,
    /// 结束符加了引号：正文原样，不展开。
    quoted: bool,
}

impl Lexer {
    /// `<<` 已经吃掉了：读结束符，记下来，正文等这一行完了再读。没有结束符的不算。
    pub(super) fn heredoc_start(&mut self) {
        let strip = self.peek(0) == Some('-');
        if strip {
            self.at += 1;
        }
        while matches!(self.peek(0), Some(' ' | '\t')) {
            self.at += 1;
        }
        let (mut end, mut quoted) = (String::new(), false);
        while let Some(c) = self.peek(0) {
            match c {
                '\'' | '"' => {
                    quoted = true;
                    self.at += 1;
                    while let Some(inner) = self.peek(0) {
                        self.at += 1;
                        if inner == c {
                            break;
                        }
                        end.push(inner);
                    }
                }
                '\\' => {
                    quoted = true;
                    self.at += 1;
                    if let Some(next) = self.peek(0) {
                        self.at += 1;
                        end.push(next);
                    }
                }
                c if c.is_whitespace() || matches!(c, ';' | '|' | '&' | '<' | '>' | '(' | ')') => {
                    break;
                }
                c => {
                    self.at += 1;
                    end.push(c);
                }
            }
        }
        if !end.is_empty() {
            self.heredocs.push(Heredoc { end, strip, quoted });
        }
    }

    /// 刚过了一个换行：记着的 heredoc 照先后读掉正文，到结束符那一行（连它的换行）为止；没有那一行的读到结尾。
    pub(super) fn heredoc_bodies(&mut self) {
        for heredoc in std::mem::take(&mut self.heredocs) {
            let start = self.at;
            let mut body_end = self.chars.len();
            while self.at < self.chars.len() {
                let line_start = self.at;
                let line_end = self.chars[line_start..]
                    .iter()
                    .position(|&c| c == '\n')
                    .map_or(self.chars.len(), |n| line_start + n);
                self.at = (line_end + 1).min(self.chars.len());
                let line: String = self.chars[line_start..line_end].iter().collect();
                let line = if heredoc.strip {
                    line.trim_start_matches('\t')
                } else {
                    line.as_str()
                };
                if line == heredoc.end {
                    body_end = line_start;
                    break;
                }
            }
            if !heredoc.quoted {
                let body: String = self.chars[start..body_end].iter().collect();
                let inner = substitutions(&body);
                self.parsed.inner.extend(inner);
            }
        }
    }
}

/// 没加引号的正文里的 `$( )`、反引号：里面的原文。
fn substitutions(body: &str) -> Vec<String> {
    let mut lexer = Lexer::new(body);
    let mut found = Vec::new();
    while let Some(c) = lexer.peek(0) {
        match c {
            '\\' => lexer.at += 2,
            '`' => found.push(lexer.until_backtick()),
            '$' if lexer.peek(1) == Some('(') => {
                lexer.at += 2;
                found.push(lexer.until_close());
            }
            _ => lexer.at += 1,
        }
    }
    found
}
