//! `web_fetch`（施工 N-1，设计 36 第一节，`docs/blueprint/tools/web_fetch.md`）：抓一个网址，直接交正文。
//!
//! 网页换成 Markdown，文字照原样（[`Pages`]）；太长的分页，一页至多 `page_chars` 个字，不止一页的末尾另起一行说第几页、一共
//! 几页，她带 `page` 接着读：同一个地址十分钟内翻页照记着的，不重抓。跳转过的另起一行说落到了哪；超了大小截了后面的，最后
//! 一页说一声。访问类别是 `read`（设计 36 第一节第 4 条：只读、不改东西，哪里都不问人），工具面上哪种会话都有。
//!
//! 给她看的字在资源目录 `software/net/web_fetch/*.txt`（进登记簿），给人看的在 `software/net/human/`。

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError, say};
use miyu_tool::{Call, Done, Progress, Running, Spec, Tool};

use crate::document::{Miss, Pages};
use crate::search::Search;

/// 这个包的编号：资源目录里的名字。
pub const PACKAGE: &str = "net";

/// 工具名。
pub const WEB_FETCH: &str = "web_fetch";

/// 给人看的标题至多几个字。
const TITLE_CHARS: usize = 60;

/// `web_fetch`。
pub(crate) struct WebFetch {
    spec: Spec,
    texts: Texts,
    pages: Arc<Pages>,
    /// 自己抓不下来的请它代抓（施工 N-4）。
    search: Arc<Search>,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    url: String,
    #[serde(default)]
    page: Option<usize>,
}

/// 给她看的几句。
struct Texts {
    more: Template,
    moved: Template,
    cut: Template,
    bad_args: Template,
    not_a_url: Template,
    blocked: Template,
    unreachable: Template,
    redirects: Template,
    challenge: Template,
    status: Template,
    unsupported: Template,
    too_complex: Template,
    empty: Template,
    not_ready: Template,
    no_page: Template,
}

impl Texts {
    fn load(resources: &Path) -> Result<Texts, LoadError> {
        let text =
            |name: &str, fields: &[&str]| load::text(resources, PACKAGE, WEB_FETCH, name, fields);
        Ok(Texts {
            more: text("more", &["page", "pages"])?,
            moved: text("moved", &["url"])?,
            cut: text("cut", &[])?,
            bad_args: text("bad-args", &["error"])?,
            not_a_url: text("not-a-url", &["url"])?,
            blocked: text("blocked", &["url"])?,
            unreachable: text("unreachable", &["url"])?,
            redirects: text("redirects", &["url"])?,
            challenge: text("challenge", &["url"])?,
            status: text("status", &["url", "status"])?,
            unsupported: text("unsupported", &["url", "type"])?,
            too_complex: text("too-complex", &["url"])?,
            empty: text("empty", &["url"])?,
            not_ready: text("not-ready", &[])?,
            no_page: text("no-page", &["page", "pages"])?,
        })
    }

    /// 一句出错：给她看 `template` 换进 `fields`，给人看的是 `key`、带同样的字段。
    fn error(template: &Template, key: &str, fields: &[(&str, &str)]) -> Done {
        Done::error(line(template, fields)).said(said(key, fields))
    }

    fn missed(&self, url: &str, miss: &Miss) -> Done {
        let at = [("url", url)];
        match miss {
            Miss::NotAUrl | Miss::Scheme => {
                Texts::error(&self.not_a_url, "web_fetch/not-a-url", &at)
            }
            Miss::Blocked => Texts::error(&self.blocked, "web_fetch/blocked", &at),
            Miss::Unreachable => Texts::error(&self.unreachable, "web_fetch/unreachable", &at),
            Miss::Redirects => Texts::error(&self.redirects, "web_fetch/redirects", &at),
            Miss::Challenge => Texts::error(&self.challenge, "web_fetch/challenge", &at),
            Miss::Status(status) => Texts::error(
                &self.status,
                "web_fetch/status",
                &[("url", url), ("status", &status.to_string())],
            ),
            Miss::Unsupported(kind) => {
                let kind = if kind.is_empty() { "?" } else { kind.as_str() };
                Texts::error(
                    &self.unsupported,
                    "web_fetch/unsupported",
                    &[("url", url), ("type", kind)],
                )
            }
            Miss::TooComplex => Texts::error(&self.too_complex, "web_fetch/too-complex", &at),
            Miss::Empty => Texts::error(&self.empty, "web_fetch/empty", &at),
            Miss::NotReady => Texts::error(&self.not_ready, "web_fetch/not-ready", &[]),
        }
    }
}

impl WebFetch {
    /// 照资源目录 `resources` 造：说明、几句字都从那里读；抓网页的家底是 `pages`。
    pub(crate) fn load(
        resources: &Path,
        pages: Arc<Pages>,
        search: Arc<Search>,
    ) -> Result<WebFetch, LoadError> {
        Ok(WebFetch {
            spec: load::spec(resources, PACKAGE, WEB_FETCH, Access::Read)?,
            texts: Texts::load(resources)?,
            pages,
            search,
        })
    }
}

/// 替谁抓（施工 MB-2）：开口的是机器主人的（[`miyu_tool::Caller::owner`]）照 `net.md` 第 5 条；别的照最严，带上系统配置里的内部域名。
/// 拿不到调用的事实的也照最严。
fn asker(call: &Call) -> crate::Asker {
    if call.caller.as_ref().is_some_and(|caller| caller.owner) {
        return crate::Asker::Owner;
    }
    let internal = call
        .settings
        .as_deref()
        .and_then(|settings| {
            crate::settings::NetSettings::from(&settings.values()).internal_domains
        })
        .unwrap_or_default();
    crate::Asker::Other { internal }
}

impl Tool for WebFetch {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let texts = &self.texts;
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => {
                    return Texts::error(
                        &texts.bad_args,
                        "web_fetch/bad-args",
                        &[("error", &error.to_string())],
                    );
                }
            };
            let asker = asker(&call);
            let mut read = self.pages.read(&args.url, &asker).await;
            if rescue::worth(&read, &self.search) {
                // 这一轮的配置（施工 N-4）：挑的来源排在前面、Exa 配了 key 的走自己的额度。
                let using = call
                    .settings
                    .as_deref()
                    .map(crate::settings::using)
                    .unwrap_or_default();
                let with = (&*self.pages, &*self.search, &using);
                read = rescue::rescue(read, &args.url, &asker, with).await;
            }
            let read = match read {
                Ok(read) => read,
                Err(miss) => return texts.missed(args.url.trim(), &miss),
            };
            let pages = split(&read.text, self.pages.page_chars());
            let page = args.page.unwrap_or(1);
            let Some(body) = page.checked_sub(1).and_then(|at| pages.get(at)) else {
                let (asked, all) = (page.to_string(), pages.len().to_string());
                return Texts::error(
                    &texts.no_page,
                    "web_fetch/no-page",
                    &[("page", &asked), ("pages", &all)],
                );
            };
            let (number, count) = (page.to_string(), pages.len().to_string());
            let mut text = String::new();
            if read.url.trim_end_matches('/') != args.url.trim().trim_end_matches('/') {
                text.push_str(&line(&texts.moved, &[("url", &read.url)]));
                text.push_str("\n\n");
            }
            text.push_str(body.trim_end());
            if pages.len() > 1 {
                text.push('\n');
                text.push_str(&line(&texts.more, &[("page", &number), ("pages", &count)]));
            }
            if read.cut && page == pages.len() {
                text.push('\n');
                text.push_str(&line(&texts.cut, &[]));
            }
            Done::ok(text).said(told(
                read.title.as_deref(),
                &number,
                &count,
                read.via.as_deref(),
            ))
        })
    }
}

/// 分页：每页至多 `most` 个字，尽量断在换行上（这一页后五分之一里最后一个换行），断不了的照字数断。
pub(crate) fn split(text: &str, most: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    let most = most.max(1);
    let mut pages = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mut end = (start + most).min(chars.len());
        if end < chars.len() {
            let floor = start + most * 4 / 5;
            if let Some(newline) = (floor..end).rev().find(|&at| chars[at] == '\n') {
                end = newline + 1;
            }
        }
        pages.push(chars[start..end].iter().collect());
        start = end;
    }
    pages
}

/// 给人看的那一句：有标题的写标题（太长的截到 [`TITLE_CHARS`] 个字），不止一页的写第几页。
fn told(title: Option<&str>, page: &str, pages: &str, via: Option<&str>) -> Said {
    let at = [("page", page), ("pages", pages)];
    let title = title.map(|title| match title.char_indices().nth(TITLE_CHARS) {
        Some((end, _)) => format!("{}…", &title[..end]),
        None => title.to_string(),
    });
    let told = match (title, pages == "1") {
        (Some(title), true) => said("web_fetch/titled", &[("title", &title)]),
        (Some(title), false) => said("web_fetch/titled-page", &[("title", &title), at[0], at[1]]),
        (None, true) => said("web_fetch/fetched", &[]),
        (None, false) => said("web_fetch/page", &at),
    };
    // 别家代抓的带上是哪一家（施工 N-4），同 `web_search` 的 `via`：头画在名字后面。
    match via {
        Some(via) => told.with("via", via.to_string()),
        None => told,
    }
}

/// 照模板换进字段，去掉末尾的换行。
fn line(template: &Template, fields: &[(&str, &str)]) -> String {
    say(template, fields).trim_end().to_string()
}

/// 给人看的说法：编号是 `software/net/<key>`，带着换进去的字段。
fn said(key: &str, fields: &[(&str, &str)]) -> Said {
    fields.iter().fold(
        Said::new(format!("software/{PACKAGE}/{key}")),
        |said, (name, value)| said.with(name, (*value).to_string()),
    )
}

mod rescue;
#[cfg(test)]
mod tests;
