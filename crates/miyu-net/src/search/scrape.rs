//! 免 key 的网页抓取兜底（施工 N-3 下，设计 36 第二节第 1 条最后一行）：读 DuckDuckGo（HTML 版）、Bing 的搜索结果页。
//!
//! 2026-10-19 实测：旧版抓的几家里 Yahoo 回 500、360 跳转、搜狗第二次就要验证码、DuckDuckGo 精简版直接挡；DuckDuckGo 的 HTML
//! 版、Bing 能用，DuckDuckGo 连问几次就弹人机验证（`anomaly.js`）。页面随时改版，所以不手写字符串扫描（旧版那样），照 HTML
//! 建树（`htmd` 底下的 html5ever），照 class 找：
//!
//! - DuckDuckGo：照文档先后，`a.result__a` 是一条的标题和链接（链接包在 `//duckduckgo.com/l/?uddg=<编码的网址>` 里），
//!   之后的 `.result__snippet` 是它的摘录。
//! - Bing：每个 `li.b_algo` 一条，`h2 > a` 是标题和链接（链接包在 `bing.com/ck/a?…&u=a1<base64url 的网址>` 里），`.b_caption`
//!   里的字是摘录。
//!
//! 弹人机验证的当限额（进冷却）：一条都没认出、页面上有验证的记号。

use std::rc::Rc;
use std::sync::LazyLock;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use htmd::{HtmlToMarkdown, Node};
use markup5ever_rcdom::NodeData;

use super::{Failure, Hit};

/// 只用来建树的那一份。
static PARSER: LazyLock<HtmlToMarkdown> = LazyLock::new(HtmlToMarkdown::new);

/// 人机验证页上的记号。
const CHALLENGES: [&str; 5] = [
    "anomaly.js",
    "challenge-form",
    "captcha",
    "/challenge/",
    "unusual traffic",
];

/// DuckDuckGo HTML 版的结果页。
pub(crate) fn duckduckgo(html: &str) -> Result<Vec<Hit>, Failure> {
    let root = tree(html)?;
    let mut hits: Vec<Hit> = Vec::new();
    for node in preorder(&root) {
        if has_class(&node, "result__a") {
            let url = attr(&node, "href")
                .map(|href| unwrap_duckduckgo(&href))
                .unwrap_or_default();
            hits.push(Hit {
                title: text(&node),
                url,
                snippet: None,
                date: None,
            });
        } else if has_class(&node, "result__snippet")
            && let Some(last) = hits.last_mut().filter(|last| last.snippet.is_none())
        {
            last.snippet = Some(text(&node)).filter(|snippet| !snippet.is_empty());
        }
    }
    found(hits, html)
}

/// Bing 的结果页。
pub(crate) fn bing(html: &str) -> Result<Vec<Hit>, Failure> {
    let root = tree(html)?;
    let hits = preorder(&root)
        .into_iter()
        .filter(|node| is(node, "li") && has_class(node, "b_algo"))
        .filter_map(|item| {
            let nodes = preorder(&item);
            let heading = nodes.iter().find(|node| is(node, "h2"))?;
            let link = preorder(heading).into_iter().find(|node| is(node, "a"))?;
            let snippet = nodes
                .iter()
                .find(|node| has_class(node, "b_caption"))
                .map(text_outside_links)
                .filter(|snippet| !snippet.is_empty());
            Some(Hit {
                title: text(&link),
                url: attr(&link, "href")
                    .map(|href| unwrap_bing(&href))
                    .unwrap_or_default(),
                snippet,
                date: None,
            })
        })
        .collect();
    found(hits, html)
}

/// 认出的几条；一条都没有、页面上有人机验证的记号的当限额。
fn found(hits: Vec<Hit>, html: &str) -> Result<Vec<Hit>, Failure> {
    let hits: Vec<Hit> = hits
        .into_iter()
        .filter(|hit| !hit.url.is_empty() && !hit.title.is_empty())
        .collect();
    if hits.is_empty() {
        let lower = html.to_ascii_lowercase();
        if CHALLENGES.iter().any(|mark| lower.contains(mark)) {
            return Err(Failure::Limited);
        }
    }
    Ok(hits)
}

/// DuckDuckGo 包着的链接：`uddg=` 那一格解码；没包的、`//` 开头的补上 `https:`。
fn unwrap_duckduckgo(href: &str) -> String {
    let full = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href.to_string()
    };
    reqwest::Url::parse(&full)
        .ok()
        .and_then(|url| {
            url.query_pairs()
                .find(|(key, _)| key == "uddg")
                .map(|(_, value)| value.into_owned())
        })
        .unwrap_or(full)
}

/// Bing 包着的链接：`u=a1` 后面是网址的 base64url（不带补齐的 `=`）；解不开的照原样。
fn unwrap_bing(href: &str) -> String {
    reqwest::Url::parse(href)
        .ok()
        .and_then(|url| {
            url.query_pairs()
                .find(|(key, _)| key == "u")
                .map(|(_, value)| value.into_owned())
        })
        .and_then(|wrapped| {
            let encoded = wrapped.strip_prefix("a1")?;
            let bytes = URL_SAFE_NO_PAD.decode(encoded.trim_end_matches('=')).ok()?;
            String::from_utf8(bytes).ok()
        })
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
        .unwrap_or_else(|| href.to_string())
}

/// 建树：解析不了的当这一家出错。
fn tree(html: &str) -> Result<Rc<Node>, Failure> {
    PARSER
        .html_to_tree(html)
        .map_err(|error| Failure::Failed(format!("unreadable page: {error}")))
}

/// 照文档先后的全部节点：自己用栈走，不递归（深的页面不把栈用光）。
fn preorder(root: &Rc<Node>) -> Vec<Rc<Node>> {
    let mut nodes = Vec::new();
    let mut stack = vec![Rc::clone(root)];
    while let Some(node) = stack.pop() {
        stack.extend(node.children.borrow().iter().rev().map(Rc::clone));
        nodes.push(node);
    }
    nodes
}

/// 是不是名叫 `name` 的元素。
fn is(node: &Rc<Node>, name: &str) -> bool {
    matches!(&node.data, NodeData::Element { name: tag, .. } if tag.local.as_ref() == name)
}

/// 元素属性 `key` 的值。
fn attr(node: &Rc<Node>, key: &str) -> Option<String> {
    match &node.data {
        NodeData::Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .find(|attr| attr.name.local.as_ref() == key)
            .map(|attr| attr.value.to_string()),
        _ => None,
    }
}

/// 元素的 class 里有没有 `class`。
fn has_class(node: &Rc<Node>, class: &str) -> bool {
    attr(node, "class").is_some_and(|classes| classes.split_whitespace().any(|one| one == class))
}

/// 节点里链接以外的字：Bing 的摘录末尾有「阅读更多」那种展开的链接（2026-10-19 实测），读的人用不上。
fn text_outside_links(node: &Rc<Node>) -> String {
    let mut parts = Vec::new();
    let mut stack = vec![Rc::clone(node)];
    while let Some(node) = stack.pop() {
        if is(&node, "a") {
            continue;
        }
        if let NodeData::Text { contents } = &node.data {
            parts.push(contents.borrow().to_string());
        }
        stack.extend(node.children.borrow().iter().rev().map(Rc::clone));
    }
    parts
        .concat()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 节点里的字：连起来，空白收拢成一个空格。
fn text(node: &Rc<Node>) -> String {
    let joined: String = preorder(node)
        .iter()
        .filter_map(|node| match &node.data {
            NodeData::Text { contents } => Some(contents.borrow().to_string()),
            _ => None,
        })
        .collect();
    joined.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests;
