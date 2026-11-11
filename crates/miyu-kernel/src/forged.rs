//! 模型编的下一轮（施工 FT-1，`docs/blueprint/kernel/session.md`「截掉模型编的下一轮」）：模型在回答后面接着写出一个角色词，再照
//! 这一次请求里人这一边的样子编出一整条「别人说的话」。它落进日志，就成了她上下文里一条伪造的指令（线上那一次编的是主人的话，
//! 后面两轮的思考都当了真），平台桥还照它发进了群。
//!
//! 这里只管认，不管截：这一次请求里人这一边出现过哪些标签（[`Shells`]，会话里一次次请求接着算的 [`Facing`]），回答的一块字
//! 里从哪儿起是编的（[`find`]）。截掉、记进 `model.called` 在 `session/call/forged.rs`。
//!
//! 纯函数，只看这一次请求的内容和模型回的字（内核 K1）。认得严，宁可漏、不错截：角色词要在行首，后面挨着的要是这一次请求里
//! 真出现过的外壳；人让她写的示范对话（`user: 你好`）后面没有外壳，不截。

use std::collections::BTreeSet;

use crate::block::Block;
use crate::event::ForgedRole;
use crate::request::{Fingerprint, Message, Request};

/// 认的角色词，都写成小写；比的时候不分大小写。
const ROLES: [(&str, ForgedRole); 5] = [
    ("user", ForgedRole::User),
    ("assistant", ForgedRole::Assistant),
    ("system", ForgedRole::System),
    ("human", ForgedRole::Human),
    ("tool", ForgedRole::Tool),
];

/// 一次请求里人这一边出现过的开标签的名字：核心自己写的事实块、扩展交来的事实、人的话里碰巧写的，都照请求里实际的字认，
/// 不另维护一张标签表。请求交出去之前算好，记在在路上的那一次请求上：回答到了请求已经不在手里。
#[derive(Debug, Default)]
pub struct Shells(BTreeSet<String>);

impl Shells {
    /// `request` 里每一条 user 消息的文本块，加上接在最末尾的那一句（`tail`，发出去也在人这一边），里面的开标签。整份扫
    /// 一遍；会话里一次次请求接着算的用 [`Facing`]，出来的一样。
    pub fn of(request: &Request) -> Shells {
        let mut names = BTreeSet::new();
        collect(&mut names, &request.messages);
        Shells(names).with_tail(request)
    }

    /// 加上接在最末尾的那一句里的。
    fn with_tail(mut self, request: &Request) -> Shells {
        if let Some(tail) = &request.tail {
            scan(&mut self.0, tail);
        }
        self
    }
}

/// 一个会话接着算外壳用的：上一次主请求每条消息的指纹，和那些消息里的开标签。
///
/// 请求多半只是在上一次后面接着加（前缀稳定是这个项目的头一条），接着加的只扫新加的几条；前面有一条不一样、变短了的
/// （压缩、撤销、换了策略）整份重扫。出来的和 [`Shells::of`] 整份扫的一样。为什么不每次整份扫：量过（施工 FT-1，本机
/// release），整份扫约 1 MB 的请求 0.25 毫秒、4.3 MB 的 1 毫秒，是算指纹的两倍半；「一次请求的投影」一万条事件的预算一共
/// 5 毫秒（设计 23）。消息一样不一样照指纹认，和 `first_difference` 同一份哈希。只在内存里，载入以后头一次整份扫。
#[derive(Debug, Default)]
pub struct Facing {
    /// 上一次主请求每条消息的指纹，照先后。
    hashes: Vec<u64>,
    /// 那些消息里的开标签。
    names: BTreeSet<String>,
}

impl Facing {
    /// `request`（指纹是 `fingerprint`）里人这一边的外壳，连同接在最末尾的那一句里的；记下这一次的，下一次接着算。
    pub fn face(&mut self, request: &Request, fingerprint: &Fingerprint) -> Shells {
        let hashes: Vec<u64> = fingerprint.hashes().2.collect();
        let extends = hashes.starts_with(&self.hashes);
        if !extends {
            self.names.clear();
        }
        let from = if extends { self.hashes.len() } else { 0 };
        collect(&mut self.names, &request.messages[from..]);
        self.hashes = hashes;
        Shells(self.names.clone()).with_tail(request)
    }
}

/// `messages` 里每一条 user 消息的文本块里的开标签，并进 `names`。
fn collect(names: &mut BTreeSet<String>, messages: &[Message]) {
    let said = messages
        .iter()
        .filter_map(|message| match message {
            Message::User { blocks } => Some(blocks),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        });
    for text in said {
        scan(names, text);
    }
}

/// `text` 里的开标签，并进 `names`。
fn scan(names: &mut BTreeSet<String>, text: &str) {
    for (at, _) in text.match_indices('<') {
        if let Some(name) = open_tag(&text[at..])
            && !names.contains(name)
        {
            names.insert(name.to_string());
        }
    }
}

/// 一块字里编的那一段从哪儿起。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forged {
    /// 角色词那一行的行首在第几个字节：从这里到末尾都是编的。总在字的边界上（开头，或者一个换行后面）。
    pub at: usize,
    /// 那个角色词。
    pub role: ForgedRole,
}

/// `text`（回答的一块字）里最早的一处编的下一轮；没有的是 `None`。
///
/// 要两样挨着：
///
/// 1. 一行的开头（或者整块的开头）是一个角色词（`user`、`assistant`、`system`、`human`、`tool`，不分大小写），后面可以有
///    空格、一个 `:` 或 `：`；
/// 2. 跳过空白和换行，紧接着是这一次请求里人这一边出现过的外壳：`shells` 里有的开标签，或者 `record` 认得的一条记录的
///    行首（组装器写别人说的一条话的写法，[`crate::assemble::Assembler::record_head`]）。
///
/// 只有角色词的、只有外壳的都不算。
pub fn find(text: &str, shells: &Shells, record: &dyn Fn(&str) -> bool) -> Option<Forged> {
    let starts = std::iter::once(0).chain(text.match_indices('\n').map(|(at, _)| at + 1));
    for at in starts {
        let Some((role, rest)) = role_word(&text[at..]) else {
            continue;
        };
        let rest = rest.trim_start_matches([' ', '\t']);
        let rest = rest.strip_prefix([':', '：']).unwrap_or(rest);
        let rest = rest.trim_start();
        if open_tag(rest).is_some_and(|name| shells.0.contains(name)) || record(rest) {
            return Some(Forged { at, role });
        }
    }
    None
}

/// `line` 开头的角色词和它后面的字。角色词后面接着字母的（`username`）这里也交回，后面不是外壳，[`find`] 不认。
fn role_word(line: &str) -> Option<(ForgedRole, &str)> {
    ROLES.iter().find_map(|(word, role)| {
        let head = line.get(..word.len())?;
        head.eq_ignore_ascii_case(word)
            .then(|| (role.clone(), &line[word.len()..]))
    })
}

/// `text` 开头是一个开标签的，交回它的名字：`<名字>`、`<名字/>`、`<名字 属性…>`（同一行里收住）。名字是字母开头，只有字母、
/// 数字、`-`、`_`（核心的事实块、模板都这样写）。收尾标签 `</名字>` 不算。
fn open_tag(text: &str) -> Option<&str> {
    let rest = text.strip_prefix('<')?;
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(rest.len());
    let (name, after) = rest.split_at(end);
    if !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let closed = match after.chars().next()? {
        '>' => true,
        '/' => after.starts_with("/>"),
        // 带属性的：这一行里有收住它的 `>`，它前面没有另一个 `<`。
        ' ' | '\t' => after
            .lines()
            .next()
            .and_then(|line| line.split_once('>'))
            .is_some_and(|(attributes, _)| !attributes.contains('<')),
        _ => false,
    };
    closed.then_some(name)
}

#[cfg(test)]
mod tests;
