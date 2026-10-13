//! 自动想起（施工 K-6 下，`docs/blueprint/knowledge.md` 第九条，19 N5）：人开的一轮，回合开始照这一轮人说的话，从她查得到、
//! 开了 `auto` 的库里挑几段带给她，宁可不带（`knowledge/eval.md`，项目主人 2026-10-13 定的挑法）：
//!
//! - **标题名**：话里出现了某一段标题的最后一段（[`NAME`] 个字以上）的带。
//! - **照意思**：照这一轮的配置算话的向量，每个库找最像的 [`WIDE`] 块，过门槛的带（`core/knowledge/recall.toml`）；表里没有的
//!   模型不算向量（远程的不白花钱），只走标题名。
//! - 模型在门槛表里的，开了 `auto` 的库在后台补缺的向量（同搜完补）：她不搜，照意思那一路也补得上。照意思那一路限时
//!   [`MEANING`]，等不到的这一轮只走标题名。
//! - 先标题名、再照意思，这一段上下文里交过的不带；一轮最多 [`TURN`] 段，联想和自动想起合起来一段上下文累计 [`SEGMENT`] 条
//!   （19 N5 共用一份预算，记忆那边数的时候也把这边的算上）。
//! - 一段两行：头一行照 `kb_search` 的那一行，下面是这一段开头 [`SNIPPET`] 个字，截了的照 `recall/cut.txt` 接省略号；要全文她
//!   自己 `kb_read`。一块不超过 [`LIMIT`] 字节，放不下的那一段和以后的不加。

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::event::ContextInjected;
use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId};
use miyu_kernel::session::Injection;
use miyu_kernel::template::Template;
use miyu_recall::associate::{SEGMENT, TURN, Thresholds, pick};
use miyu_store::knowledge::Hit;
use miyu_store::resources::ResourceRoot;
use miyu_tool::load;

use super::{Shelves, report};
use crate::TARGET;
use crate::blocking::blocking;
use crate::memory::Using;

/// 注入那一块的模块。
pub(crate) const MODULE: &str = "knowledge";

/// 注入那一块的类别：同记忆的联想（记忆那边照它把这边带过的也数进一段上下文的累计）。
pub(crate) const KIND: &str = "recall";

/// 一块最多几个字节：一段头一行加 120 个字约 400 字节，两三段放得下，同记忆联想一块的量级。
const LIMIT: usize = 600;

/// 一段带开头几个字。
const SNIPPET: usize = 120;

/// 人说的话最多拿前几个字（同记忆的联想：人可能贴进来一整个文件）。
const SAID: usize = 1000;

/// 每个库照意思找几块再挑：交过的挑掉以后还够。
const WIDE: usize = 32;

/// 标题名至少几个字：一个字的（「猫」）到处都撞得上。
const NAME: usize = 2;

/// 照意思那一路最晚等到什么时候：从自动想起开始数。比回合开始的挂接点（`actor/model.rs` 的 `HOOKS`，300 毫秒）短，留出拼块的
/// 工夫：本机模型冷启动的那一轮（真 bge 实测头一轮整块超时）、那一家回得慢的、读段读得慢的，这一轮只走标题名，标题名不跟着
/// 整块超时丢掉。从开始数而不是从问向量数：读段慢了（头一回补、慢机器）照意思那一路少等，不把整块拖过挂接点。
const MEANING: Duration = Duration::from_millis(200);

/// 自动想起要的几样：外壳、截短的写法、头一行（同 `kb_search`）、门槛表。
#[derive(Debug)]
pub(crate) struct RecallTexts {
    open: Template,
    close: Template,
    cut: Template,
    hit: Template,
    plain: Template,
    page: Template,
    thresholds: Thresholds,
}

/// 从资源目录读自动想起要的几样；读不出来的这个会话不自动想起，记一行。
pub(crate) fn texts(resources: &ResourceRoot) -> Option<Arc<RecallTexts>> {
    match load(resources) {
        Ok(texts) => Some(Arc::new(texts)),
        Err(error) => {
            tracing::warn!(target: TARGET, error = error.as_str(), "knowledge recall texts unreadable");
            None
        }
    }
}

fn load(resources: &ResourceRoot) -> Result<RecallTexts, String> {
    let thresholds = resources
        .knowledge_table("recall")
        .map_err(|error| error.to_string())
        .and_then(|text| {
            Thresholds::parse(&text).map_err(|why| format!("core/knowledge/recall.toml：{why}"))
        })?;
    let text = |tool: &str, name: &str, fields: &[&str]| {
        load::text(resources.path(), MODULE, tool, name, fields).map_err(|error| error.to_string())
    };
    Ok(RecallTexts {
        open: text("recall", "open", &[])?,
        close: text("recall", "close", &[])?,
        cut: text("recall", "cut", &["text"])?,
        hit: text("kb_search", "hit", &["kb", "path", "heading"])?,
        plain: text("kb_search", "plain", &["kb", "path"])?,
        page: text("kb_search", "page", &["kb", "path", "page"])?,
        thresholds,
    })
}

impl Shelves {
    /// 这一轮自动想起带什么（`said` 是人说的话，`present` 是内核交来的这一段上下文里模块注入过的）。会话没交自动想起的字的、
    /// 一个开了 `auto` 的库都没有的、一段都没挑上的，没有。
    pub(crate) async fn recall(
        self: &Arc<Self>,
        using: &Using,
        said: &str,
        present: &[Present],
    ) -> Option<Injection> {
        let started = tokio::time::Instant::now();
        let texts = self.recall.clone()?;
        let said: String = said.chars().take(SAID).collect();
        let (seen, brought) = seen(present);
        let room = room(brought);
        if room == 0 {
            return None;
        }
        let shelves = Arc::clone(self);
        let sections = match blocking(move || shelves.auto_sections()).await {
            Ok(sections) => sections,
            Err(error) => {
                tracing::warn!(target: TARGET, error = error.as_str(), "knowledge recall not read");
                return None;
            }
        };
        if sections.is_empty() {
            return None;
        }
        let threshold = self.threshold(using, &texts);
        if threshold.is_some() {
            let ids: BTreeSet<String> = sections.iter().map(|hit| hit.kb.clone()).collect();
            super::fill(self, using, ids.into_iter().collect());
        }
        let mut chosen = named(&sections, &said, &seen, room);
        if let Some(threshold) = threshold
            && chosen.len() < room
            && let Some(near) = self
                .closer(using, &said, &sections, started + MEANING)
                .await
        {
            let skip: BTreeSet<String> = seen.iter().chain(&chosen).cloned().collect();
            let more = pick(&near, threshold, &skip, brought + chosen.len());
            chosen.extend(more.into_iter().take(room - chosen.len()));
        }
        let picked: Vec<&Hit> = chosen
            .iter()
            .filter_map(|key| sections.iter().find(|hit| self::key(hit) == *key))
            .collect();
        render(&texts, &picked)
    }

    /// 她查得到的、开了 `auto` 的库的每一段（先照文件补；跳过的文档照「出错」记一行）。
    fn auto_sections(&self) -> Result<Vec<Hit>, String> {
        let mut sections = Vec::new();
        for id in self.bases()? {
            if !self
                .knowledge
                .auto(&self.account, &id)
                .map_err(|error| error.to_string())?
            {
                continue;
            }
            let (hits, skipped) = self
                .knowledge
                .sections(&self.account, &id)
                .map_err(|error| error.to_string())?;
            report(&skipped);
            sections.extend(hits);
        }
        Ok(sections)
    }

    /// 照意思那一路的门槛：这一轮的模型在门槛表里才有。没有的只走标题名，不算话的向量、也不补段的向量（用不上，远程的不白花
    /// 钱）。
    fn threshold(&self, using: &Using, texts: &RecallTexts) -> Option<f32> {
        let model = self.vectors.as_ref()?.model(using)?;
        texts.thresholds.of(&model)
    }

    /// 照意思，最晚等到 `deadline`（自动想起开始以后 [`MEANING`]）：过了的这一轮没有，记一行 `DEBUG`（算向量的那一件照样算完，
    /// 下一轮用得上）。
    async fn closer(
        &self,
        using: &Using,
        said: &str,
        sections: &[Hit],
        deadline: tokio::time::Instant,
    ) -> Option<Vec<(String, f32)>> {
        match tokio::time::timeout_at(deadline, self.close(using, said, sections)).await {
            Ok(near) => near,
            Err(_) => {
                tracing::debug!(target: TARGET, "knowledge recall by meaning timed out");
                None
            }
        }
    }

    /// 照意思：算话的向量，`sections` 所在的每个库找最像的几块（键、相似度，最像的在前）。算不出的、读不了的没有。
    async fn close(
        &self,
        using: &Using,
        said: &str,
        sections: &[Hit],
    ) -> Option<Vec<(String, f32)>> {
        let vectors = self.vectors.as_ref()?;
        let near = vectors.query(using, said).await?;
        let ids: BTreeSet<String> = sections.iter().map(|hit| hit.kb.clone()).collect();
        let (knowledge, account) = (Arc::clone(&self.knowledge), self.account.clone());
        let found = blocking(move || {
            let mut found: Vec<(String, f32)> = Vec::new();
            for id in ids {
                let index = knowledge.index_of(&account, &id);
                let hits = index
                    .nearest(&near.model, &near.vector, WIDE)
                    .map_err(|error| error.to_string())?;
                found.extend(
                    hits.into_iter()
                        .map(|hit| (format!("{id}/{}", hit.key), hit.similar)),
                );
            }
            Ok::<_, String>(found)
        })
        .await;
        let mut found = match found {
            Ok(found) => found,
            Err(error) => {
                tracing::warn!(target: TARGET, error = error.as_str(), "knowledge recall not read");
                return None;
            }
        };
        found.sort_by(|a, b| b.1.total_cmp(&a.1));
        Some(found)
    }
}

/// 这一轮还能带几段：一轮最多 [`TURN`]，也不超过一段上下文剩下的名额（[`SEGMENT`] 减去联想、自动想起已经带过的 `brought`）。
fn room(brought: usize) -> usize {
    TURN.min(SEGMENT.saturating_sub(brought))
}

/// 标题名：话 `said` 里出现了标题路径最后一段（[`NAME`] 个字以上）的那几段，照索引里的先后，`seen` 里的（这一段上下文里交过的）
/// 不要，最多 `room` 段。
fn named(sections: &[Hit], said: &str, seen: &BTreeSet<String>, room: usize) -> Vec<String> {
    let mut chosen: Vec<String> = Vec::new();
    for hit in sections {
        let key = key(hit);
        let leaf = hit.heading.rsplit(" › ").next().unwrap_or_default();
        if leaf.chars().count() >= NAME
            && said.contains(leaf)
            && !seen.contains(&key)
            && !chosen.contains(&key)
        {
            chosen.push(key);
        }
    }
    chosen.truncate(room);
    chosen
}

/// 一段在 `refs` 里的写法：`<库>/<键>`。
fn key(hit: &Hit) -> String {
    format!("{}/{}#{}", hit.kb, hit.path, hit.chunk)
}

/// 这一段上下文里自动想起交过的几段，和联想、自动想起合起来带过几条（19 N5 共用一份预算）。
fn seen(present: &[Present]) -> (BTreeSet<String>, usize) {
    let recalled = |present: &&Present| present.kind.as_str() == KIND;
    let seen = present
        .iter()
        .filter(recalled)
        .filter(|present| present.module.as_str() == MODULE)
        .flat_map(|present| present.refs.iter().cloned())
        .collect();
    let brought = present
        .iter()
        .filter(recalled)
        .filter(|present| matches!(present.module.as_str(), MODULE | crate::memory::MODULE))
        .map(|present| present.refs.len())
        .sum();
    (seen, brought)
}

/// 拼那一块：外壳加一段两行，照先后加，加上这一段就超过 [`LIMIT`] 字节的停；一段都放不下的没有。
fn render(texts: &RecallTexts, picked: &[&Hit]) -> Option<Injection> {
    let say = |template: &Template, fields: &[(&str, &str)]| {
        load::say(template, fields).trim_end().to_string()
    };
    let (open, close) = (say(&texts.open, &[]), say(&texts.close, &[]));
    let mut used = open.len() + close.len() + 3;
    let (mut rows, mut refs) = (Vec::new(), Vec::new());
    for hit in picked {
        let head = match (hit.page, hit.heading.is_empty()) {
            (Some(page), _) => say(
                &texts.page,
                &[
                    ("kb", &hit.kb),
                    ("path", &hit.path),
                    ("page", &page.to_string()),
                ],
            ),
            (None, true) => say(&texts.plain, &[("kb", &hit.kb), ("path", &hit.path)]),
            (None, false) => say(
                &texts.hit,
                &[
                    ("kb", &hit.kb),
                    ("path", &hit.path),
                    ("heading", &hit.heading),
                ],
            ),
        };
        let flat = hit.text.split_whitespace().collect::<Vec<_>>().join(" ");
        let body = match flat.chars().count() > SNIPPET {
            true => say(
                &texts.cut,
                &[("text", &flat.chars().take(SNIPPET).collect::<String>())],
            ),
            false => flat,
        };
        let row = format!("{head}\n{body}");
        if used + row.len() + 1 > LIMIT {
            break;
        }
        used += row.len() + 1;
        rows.push(row);
        refs.push(key(hit));
    }
    if rows.is_empty() {
        return None;
    }
    Some(Injection {
        module: ModuleId::parse(MODULE).expect("合写法"),
        fact: ContextInjected {
            kind: FactKind::parse(KIND).expect("合写法"),
            text: format!("{open}\n{}\n{close}\n", rows.join("\n")),
            refs,
        },
    })
}

#[cfg(test)]
mod tests;
