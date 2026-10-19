//! 扩展的搜索来源（施工 N-5，设计 36 第二节第 5 条）：扩展在清单里写 `[[search]]`，下拉里的值是 `<包>/<编号>`；人选了它，
//! 先问它，它用不了照出厂的顺序兜底。不进没配置时的顺序：别人的程序不会在人不知道的时候拿到搜索词。
//!
//! 怎么问在上面一层（核心起来以后装进来的 [`SearchPort`]，`miyu-endpoint` 的 `search.query`），这里只管先后、冷却和收拾，
//! 和出厂的几家同一套：报限额的照台阶冷却，成功一次清零。

use std::future::Future;
use std::pin::Pin;
use std::time::Instant;

use super::{Found, Hit, Missed, Query, Ready, tidy};
use crate::TARGET;

/// 扩展这一次答的：给人看的名字、几条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    /// 来源给人看的名字（清单里写的）。
    pub name: String,
    /// 搜到的几条。
    pub hits: Vec<Hit>,
}

/// 扩展这一次没答成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unanswered {
    /// 它说限额了：照台阶冷却。
    Limited,
    /// 别的：没开、没连上、超时、出错，原因一句英文。
    Failed(String),
}

/// 问扩展的端口：核心起来以后装进 [`super::Search::attach`]。
pub trait SearchPort: Send + Sync {
    /// 问来源 `source`（`<包>/<编号>`）搜 `query`，要 `want` 条。
    fn query<'a>(
        &'a self,
        source: &'a str,
        query: &'a Query,
        want: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Answered, Unanswered>> + Send + 'a>>;
}

/// 下拉里的值是不是扩展的来源：出厂的几家不带 `/`。
pub(crate) fn is_extension(source: &str) -> bool {
    source.contains('/')
}

impl Ready {
    /// 问挑了的扩展来源 `source`：冷却着的、端口没装的不问（交回没有）；成了的交回，没成的交回原因。
    pub(super) async fn ask_extension(
        &self,
        port: &dyn SearchPort,
        source: &str,
        query: &Query,
    ) -> Option<Result<Found, Missed>> {
        if self.cooling(source).is_some() {
            return None;
        }
        Some(match port.query(source, query, self.rules.results).await {
            Ok(answered) => {
                self.cooldown().succeeded(source);
                let hits = tidy(
                    answered.hits,
                    query,
                    self.rules.results,
                    self.rules.excerpt_chars,
                );
                Ok(Found {
                    source: answered.name,
                    hits,
                })
            }
            Err(Unanswered::Limited) => {
                self.cooldown().limited(source, Instant::now());
                tracing::warn!(target: TARGET, source, "web search source limited");
                Err(Missed::Failed(format!("{source} is rate limited")))
            }
            Err(Unanswered::Failed(why)) => {
                tracing::warn!(target: TARGET, source, why = why.as_str(), "web search source failed");
                Err(Missed::Failed(format!("{source}: {why}")))
            }
        })
    }
}
