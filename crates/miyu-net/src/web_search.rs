//! `web_search`（施工 N-2 上，设计 36 第一节第 2 条，`docs/blueprint/tools/web_search.md`）：搜网页，交回几条标题、网址、
//! 日期、摘录。
//!
//! 给她看的一条是 `[1] 标题` 一行、网址（有日期的接在后面）一行，下面接摘录原样，条和条之间空一行；结果里不写是哪一家
//! （设计 36 第一节第 2 条：模型用不上）。给人看的那一句写几条、是哪一家（第三节第 2 条）。给头的 `detail` 是通用形状
//! `links`（施工 W-11 补）。访问类别 `read`，哪里都不问人（第一节第 4 条）。

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::detail::{Detail, Link};
use miyu_tool::load::{self, LoadError, say};
use miyu_tool::{Call, Done, Progress, Running, Spec, Tool};

use crate::search::{Found, Missed, Query, Search};
use crate::settings::SearchSettings;
use crate::web_fetch::PACKAGE;

/// 工具名。
pub const WEB_SEARCH: &str = "web_search";

/// `web_search`。
pub(crate) struct WebSearch {
    spec: Spec,
    texts: Texts,
    search: Arc<Search>,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    query: String,
    #[serde(default)]
    allowed_domains: Vec<String>,
    #[serde(default)]
    blocked_domains: Vec<String>,
}

/// 给她看的几句。
struct Texts {
    hit: Template,
    hit_dated: Template,
    nothing: Template,
    bad_args: Template,
    failed: Template,
    cooling: Template,
    not_ready: Template,
}

impl Texts {
    fn load(resources: &Path) -> Result<Texts, LoadError> {
        let text =
            |name: &str, fields: &[&str]| load::text(resources, PACKAGE, WEB_SEARCH, name, fields);
        Ok(Texts {
            hit: text("hit", &["n", "title", "url"])?,
            hit_dated: text("hit-dated", &["n", "title", "url", "date"])?,
            nothing: text("nothing", &["query"])?,
            bad_args: text("bad-args", &["error"])?,
            failed: text("failed", &["why"])?,
            cooling: text("cooling", &[])?,
            not_ready: text("not-ready", &[])?,
        })
    }
}

impl WebSearch {
    /// 照资源目录 `resources` 造：说明、几句字从那里读；搜网页的家底是 `search`。
    pub(crate) fn load(resources: &Path, search: Arc<Search>) -> Result<WebSearch, LoadError> {
        Ok(WebSearch {
            spec: load::spec(resources, PACKAGE, WEB_SEARCH, Access::Read)?,
            texts: Texts::load(resources)?,
            search,
        })
    }

    /// 搜成了的：几条写给她，给人一句，给头一份链接列表。
    fn found(&self, query: &str, found: Found) -> Done {
        let texts = &self.texts;
        if found.hits.is_empty() {
            return Done::ok(line(&texts.nothing, &[("query", query)]))
                .said(said("web_search/nothing", &[("source", &found.source)]));
        }
        let blocks: Vec<String> = found
            .hits
            .iter()
            .enumerate()
            .map(|(at, hit)| {
                let n = (at + 1).to_string();
                let title = if hit.title.is_empty() {
                    hit.url.as_str()
                } else {
                    hit.title.as_str()
                };
                let head = match &hit.date {
                    Some(date) => line(
                        &texts.hit_dated,
                        &[
                            ("n", &n),
                            ("title", title),
                            ("url", &hit.url),
                            ("date", date),
                        ],
                    ),
                    None => line(
                        &texts.hit,
                        &[("n", &n), ("title", title), ("url", &hit.url)],
                    ),
                };
                match &hit.snippet {
                    Some(snippet) => format!("{head}\n{snippet}"),
                    None => head,
                }
            })
            .collect();
        let count = found.hits.len().to_string();
        let key = if found.hits.len() == 1 {
            "web_search/found/one"
        } else {
            "web_search/found"
        };
        let detail = Detail::Links {
            links: found
                .hits
                .iter()
                .map(|hit| Link {
                    title: hit.title.clone(),
                    url: hit.url.clone(),
                    snippet: hit.snippet.clone(),
                    site: None,
                    date: hit.date.clone(),
                })
                .collect(),
        };
        Done::ok(blocks.join("\n\n"))
            .said(said(key, &[("hits", &count), ("source", &found.source)]))
            .detail(detail.to_value())
    }

    fn missed(&self, missed: &Missed) -> Done {
        let texts = &self.texts;
        match missed {
            Missed::Failed(why) => Done::error(line(&texts.failed, &[("why", why)]))
                .said(said("web_search/failed", &[("why", why)])),
            Missed::Cooling => {
                Done::error(line(&texts.cooling, &[])).said(said("web_search/cooling", &[]))
            }
            Missed::NotReady => {
                Done::error(line(&texts.not_ready, &[])).said(said("web_search/not-ready", &[]))
            }
        }
    }
}

impl Tool for WebSearch {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) if !args.query.trim().is_empty() => args,
                Ok(_) => return bad_args(&self.texts, "missing query"),
                Err(error) => return bad_args(&self.texts, &error.to_string()),
            };
            let query = Query {
                text: args.query.trim().to_string(),
                allowed: args.allowed_domains,
                blocked: args.blocked_domains,
            };
            // 设置里挑的来源（施工 N-2 下）：这一轮冻结的配置；没带的（测试里的假调用）照出厂的顺序。
            let source = call
                .settings
                .as_ref()
                .map(|settings| SearchSettings::from(&settings.values()).source);
            match self.search.search(&query, source.as_deref()).await {
                Ok(found) => self.found(&query.text, found),
                Err(missed) => self.missed(&missed),
            }
        })
    }
}

/// 参数不对。
fn bad_args(texts: &Texts, error: &str) -> Done {
    Done::error(line(&texts.bad_args, &[("error", error)]))
        .said(said("web_search/bad-args", &[("error", error)]))
}

/// 照模板换进字段，去掉末尾的换行。
fn line(template: &Template, fields: &[(&str, &str)]) -> String {
    say(template, fields).trim_end().to_string()
}

/// 给人看的说法：编号是 `software/net/<key>`。
fn said(key: &str, fields: &[(&str, &str)]) -> Said {
    fields.iter().fold(
        Said::new(format!("software/{PACKAGE}/{key}")),
        |said, (name, value)| said.with(name, (*value).to_string()),
    )
}
