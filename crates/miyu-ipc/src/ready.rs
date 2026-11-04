//! 核心说「好了」的那一行（`docs/designs/12-进程形态与分发.md` 第二节「拉起时的握手」，施工 3-9 上）：
//! 核心起来以后往标准输出写一行，拉起它的头等着读，不轮询。

/// 核心写的那一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ready {
    /// 好了：在套接字上等连接了。
    Ready,
    /// 已经有一个核心在跑：这一个走了，头去连那一个。
    Running,
    /// 起不来。
    Failed {
        /// 认得出是哪一种的带代码（施工 LC-1）：小写字母开头，只有小写字母、数字、`_`。头照它从给人看的字里取
        /// `start/<代码>`，照自己的语言说；没有的照原因说。
        code: Option<String>,
        /// 原因，照原样给人看。
        reason: String,
    },
}

impl Ready {
    /// 起不来，没有代码。
    pub fn failed(reason: impl Into<String>) -> Ready {
        Ready::Failed {
            code: None,
            reason: reason.into(),
        }
    }

    /// 写成一行，带换行。原因里的换行换成空格：只有一行。有代码的写成 `error <代码>: <原因>`。
    pub fn line(&self) -> String {
        match self {
            Ready::Ready => "ready\n".to_string(),
            Ready::Running => "running\n".to_string(),
            Ready::Failed { code, reason } => {
                let reason = reason.replace(['\r', '\n'], " ");
                match code {
                    Some(code) => format!("error {code}: {reason}\n"),
                    None => format!("error {reason}\n"),
                }
            }
        }
    }

    /// 从一行读回来。读不懂的当起不来，原因就是这一行。`error ` 后面冒号前是合写法的代码的，读成代码。
    pub fn parse(line: &str) -> Ready {
        let line = line.trim_end_matches(['\r', '\n']);
        match line {
            "ready" => Ready::Ready,
            "running" => Ready::Running,
            _ => match line.strip_prefix("error ") {
                Some(rest) => match rest.split_once(": ") {
                    Some((code, reason)) if is_code(code) => Ready::Failed {
                        code: Some(code.to_string()),
                        reason: reason.to_string(),
                    },
                    _ => Ready::failed(rest),
                },
                None => Ready::failed(line),
            },
        }
    }
}

/// 代码的写法：小写字母开头，只有小写字母、数字、`_`。
fn is_code(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests;
