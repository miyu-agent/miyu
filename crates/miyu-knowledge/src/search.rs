//! `kb_search`（施工 K-1 下）：照关键词搜她查得到的库。一块一段：头一行是哪个库、哪个文档、标题路径（照模板，换进去的转义），
//! 下面是这一块的字原样（文档是数据，同 `read` 交回文件的内容），段和段之间空一行；什么都没有说一句。查哪几个库、怎么合是
//! 端口的事，这里只照端口交回的写。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Call, Done, KB_SEARCH, Progress, Running, Spec, Tool};

use crate::PACKAGE;
use crate::texts::{Texts, line, said};

/// `kb_search`。
pub(crate) struct Search {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    query: String,
    #[serde(default)]
    kb: Option<String>,
}

impl Search {
    /// 照资源目录 `resources` 里的说明造，几句是 `texts`。
    pub(crate) fn load(resources: &Path, texts: Texts) -> Result<Search, LoadError> {
        Ok(Search {
            spec: load::spec(resources, PACKAGE, KB_SEARCH, Access::Read)?,
            texts,
        })
    }
}

impl Tool for Search {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let texts = &self.texts;
            let all = texts.all();
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.bad_args(&error),
            };
            let Some(port) = call.knowledge.as_ref() else {
                return texts.off();
            };
            let hits = match port.search(args.query, args.kb).await {
                Ok(hits) => hits,
                Err(refused) => return texts.refused(&refused),
            };
            // 给头看的那一份（施工 W-11，`docs/blueprint/knowledge.md`「她的工具」）：没有标题的、不是 PDF 的不写那一格。
            let detail = serde_json::json!({"hits": hits.iter().map(|hit| {
                let mut item = serde_json::json!({"kb": hit.kb, "path": hit.path, "snippet": hit.text.trim_end()});
                if !hit.heading.is_empty() {
                    item["heading"] = serde_json::Value::String(hit.heading.clone());
                }
                if let Some(page) = hit.page {
                    item["page"] = serde_json::Value::from(page);
                }
                item
            }).collect::<Vec<_>>()});
            if hits.is_empty() {
                return Done::ok(line(&all.nothing, &[]))
                    .said(said("kb_search/nothing", &[]))
                    .data(detail.clone())
                    .detail(detail);
            }
            let passages: Vec<String> = hits
                .iter()
                .map(|hit| {
                    let head = match (hit.page, hit.heading.is_empty()) {
                        // PDF 的那一块说第几页（施工 K-5）。
                        (Some(page), _) => line(
                            &all.page,
                            &[
                                ("kb", &hit.kb),
                                ("path", &hit.path),
                                ("page", &page.to_string()),
                            ],
                        ),
                        (None, true) => line(&all.plain, &[("kb", &hit.kb), ("path", &hit.path)]),
                        (None, false) => line(
                            &all.hit,
                            &[
                                ("kb", &hit.kb),
                                ("path", &hit.path),
                                ("heading", &hit.heading),
                            ],
                        ),
                    };
                    format!("{head}\n{}", hit.text.trim_end())
                })
                .collect();
            let count = hits.len().to_string();
            Done::ok(passages.join("\n\n"))
                .said(said("kb_search/found", &[("hits", &count)]))
                .data(detail.clone())
                .detail(detail)
        })
    }
}
