//! 抓到的网页换成给她读的字（施工 N-1，设计 36 第一节第 3 条）：认是什么、照字符集解成字、HTML 换成 Markdown。
//!
//! - 认种类照媒体类型：HTML 换成 Markdown；纯文字、Markdown、JSON、XML 这些照原样；别的（图、PDF、压缩包）不收。服务器没说
//!   种类的照内容猜：开头像 HTML 的当 HTML，整份是 UTF-8 的当文字。
//! - 字符集照回应头的 `charset`，没写的看 HTML 开头的 `<meta charset>`，再没有照 UTF-8（读不了的字换成替换符）。中文的老站
//!   常是 GBK，不照它解就是一屏乱码。
//! - HTML 换成 Markdown 用 `htmd`：脚本、样式、导航、页脚、表单这些对读正文没用的整段扔掉。先建树、数一遍节点和深度，太大、
//!   太深的不换：换的时候是递归的，一份故意嵌套几万层的页面能把线程的栈用光（旧版吃过的亏，`html_conversion.rs`）。
//! - 有 `<main>` 的只换第一个 `<main>`，没有的、只有一个 `<article>` 的只换它：维基、文档站的语言列表、侧栏、页头都在它外面，
//!   又不全是 `<nav>`（2026-10-18 照维基百科、ArchWiki 实测）。换完去掉没有字的链接（`[](#)`，图标按钮换出来的）。
//! - 换之前剪掉读的人看不到、用不上的：带 `hidden`、`aria-hidden="true"` 的，`role` 是导航、页头、页脚、搜索、侧栏的，
//!   链接带 `hreflang` 的列表项（换语言的那一串：维基的几十种语言在 `<main>` 里头）。只照属性认，不照 class 猜。

use std::rc::Rc;
use std::sync::LazyLock;

use htmd::{HtmlToMarkdown, Node};
use markup5ever_rcdom::NodeData;

/// 树里至多几个节点。
const MOST_NODES: usize = 200_000;

/// 树至多几层。
const MOST_DEPTH: usize = 160;

/// 认出的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// 网页：换成 Markdown。
    Html,
    /// 文字：照原样。
    Text,
}

/// 换不成。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unconverted {
    /// 树太大、太深。
    TooComplex,
    /// 解析不了。
    Unparsed,
}

/// 照媒体类型 `media`（小写、去掉了参数）认；没写的照内容 `bytes` 猜。不收的交回 `None`。
pub(crate) fn kind(media: &str, bytes: &[u8]) -> Option<Kind> {
    let text_like = media.starts_with("text/")
        || matches!(
            media,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-yaml"
        )
        || media.ends_with("+json")
        || media.ends_with("+xml");
    match media {
        "text/html" | "application/xhtml+xml" => Some(Kind::Html),
        _ if text_like => Some(Kind::Text),
        "" | "application/octet-stream" => guess(bytes),
        _ => None,
    }
}

/// 服务器没说种类：开头（去掉空白、BOM）是 `<` 又提到 html 的当网页；整份是 UTF-8、没有 NUL 的当文字。
fn guess(bytes: &[u8]) -> Option<Kind> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
    let start = head.trim_start_matches('\u{feff}').trim_start();
    if start.starts_with('<') && (start.contains("<html") || start.starts_with("<!doctype html")) {
        return Some(Kind::Html);
    }
    (std::str::from_utf8(bytes).is_ok() && !bytes.contains(&0)).then_some(Kind::Text)
}

/// 照字符集解成字：回应头写的 `charset`，网页再看开头的 `<meta>`，都没有照 UTF-8。认不出的字符集照 UTF-8。
pub(crate) fn decode(bytes: &[u8], charset: Option<&str>, kind: Kind) -> String {
    let label = charset
        .map(str::to_string)
        .or_else(|| (kind == Kind::Html).then(|| meta_charset(bytes)).flatten());
    let encoding = label
        .and_then(|label| encoding_rs::Encoding::for_label(label.trim().as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    let (text, _, _) = encoding.decode(bytes);
    text.into_owned()
}

/// HTML 开头 2048 个字节里 `<meta charset="…">` 或 `content="…; charset=…"` 写的字符集。
fn meta_charset(bytes: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]).to_ascii_lowercase();
    let at = head.find("charset=")? + "charset=".len();
    let value: String = head[at..]
        .trim_start_matches(['"', '\''])
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
        .collect();
    (!value.is_empty()).then_some(value)
}

/// 网页的标题：`<title>` 里的字，去掉前后空白、换行，常见的几个实体换回来。没有的交回 `None`。
pub(crate) fn title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = start + lower[start..].find("</title")?;
    let text: String = html[start..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let text = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    (!text.is_empty()).then_some(text)
}

/// 换的那一份：读正文没用的整段扔掉。
static CONVERTER: LazyLock<HtmlToMarkdown> = LazyLock::new(|| {
    HtmlToMarkdown::builder()
        .skip_tags(vec![
            "head", "script", "style", "noscript", "template", "svg", "canvas", "iframe", "object",
            "embed", "nav", "footer", "form", "button", "select", "dialog",
        ])
        .build()
});

/// HTML 换成 Markdown：先建树、查大小和深度，再换；连着的空行并成一行。
pub(crate) fn markdown(html: &str) -> Result<String, Unconverted> {
    let tree = CONVERTER
        .html_to_tree(html)
        .map_err(|_| Unconverted::Unparsed)?;
    if !small_enough(&tree) {
        return Err(Unconverted::TooComplex);
    }
    prune(&tree);
    let body = body(&tree);
    Ok(tidy(&drop_empty_links(&CONVERTER.tree_to_markdown(&body))))
}

/// ARIA 里和 `<nav>`、`<header>`、`<footer>`、`<aside>`、搜索框同一类的 `role`。
const CHROME_ROLES: [&str; 5] = [
    "navigation",
    "banner",
    "contentinfo",
    "search",
    "complementary",
];

/// 剪掉读的人看不到、用不上的（见开头）：自己用栈走，不递归。
fn prune(root: &Rc<Node>) {
    let mut stack = vec![Rc::clone(root)];
    while let Some(node) = stack.pop() {
        node.children.borrow_mut().retain(|child| !chrome(child));
        stack.extend(node.children.borrow().iter().map(Rc::clone));
    }
}

/// 该剪掉的一个元素。
fn chrome(node: &Rc<Node>) -> bool {
    let NodeData::Element { name, attrs, .. } = &node.data else {
        return false;
    };
    let attrs = attrs.borrow();
    let attr = |key: &str| {
        attrs
            .iter()
            .find(|attr| attr.name.local.as_ref() == key)
            .map(|attr| attr.value.as_ref().trim().to_ascii_lowercase())
    };
    if attr("hidden").is_some() || attr("aria-hidden").as_deref() == Some("true") {
        return true;
    }
    if attr("role").is_some_and(|role| CHROME_ROLES.contains(&role.as_str())) {
        return true;
    }
    name.local.as_ref() == "li"
        && elements(node, "a")
            .iter()
            .any(|link| has_attr(link, "hreflang"))
}

/// 元素带不带属性 `key`。
fn has_attr(node: &Rc<Node>, key: &str) -> bool {
    matches!(&node.data, NodeData::Element { attrs, .. }
        if attrs.borrow().iter().any(|attr| attr.name.local.as_ref() == key))
}

/// 正文在哪：第一个 `<main>`；没有的，只有一个 `<article>` 的是它；都没有的是整份。
fn body(root: &Rc<Node>) -> Rc<Node> {
    let mains = elements(root, "main");
    if let Some(main) = mains.into_iter().next() {
        return main;
    }
    let mut articles = elements(root, "article");
    if articles.len() == 1 {
        return articles.remove(0);
    }
    Rc::clone(root)
}

/// 树里名叫 `name` 的元素，照文档里的先后；自己用栈走，不递归。
fn elements(root: &Rc<Node>, name: &str) -> Vec<Rc<Node>> {
    let mut found = Vec::new();
    let mut stack = vec![Rc::clone(root)];
    while let Some(node) = stack.pop() {
        if let NodeData::Element { name: tag, .. } = &node.data
            && tag.local.as_ref() == name
        {
            found.push(Rc::clone(&node));
            continue;
        }
        stack.extend(node.children.borrow().iter().rev().map(Rc::clone));
    }
    found
}

/// 去掉没有字的链接 `[](…)`：图标按钮、锚点换出来的，读的人用不上。
fn drop_empty_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("[](") {
        let Some(end) = rest[at + 3..].find(')') else {
            break;
        };
        out.push_str(&rest[..at]);
        rest = &rest[at + 3 + end + 1..];
    }
    out.push_str(rest);
    out
}

/// 节点不超过 [`MOST_NODES`]、不深过 [`MOST_DEPTH`]：自己用栈数，不递归。
fn small_enough(root: &Rc<Node>) -> bool {
    let mut stack = vec![(Rc::clone(root), 0_usize)];
    let mut nodes = 0_usize;
    while let Some((node, depth)) = stack.pop() {
        nodes += 1;
        if nodes > MOST_NODES || depth > MOST_DEPTH {
            return false;
        }
        for child in node.children.borrow().iter() {
            stack.push((Rc::clone(child), depth + 1));
        }
    }
    true
}

/// 每行去掉行尾空白，三个以上连着的换行并成一个空行，去掉前后空白。
pub(crate) fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank = 0_usize;
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests;
