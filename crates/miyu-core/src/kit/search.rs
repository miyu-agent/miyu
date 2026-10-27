//! 问扩展的搜索来源（施工 N-5）：`miyu_net::SearchPort` 的实现，经 `miyu_endpoint::search_sources::query` 反向调扩展的
//! `search.query`，回应读成搜到的几条。核心起来以后装进联网包的搜索家底（[`super::Kit::attach`]），拿着核心的弱引用：核心
//! 没了的当没开。
//!
//! `search.query` 的参数：`source`（`<包>/<编号>`）、`query`、`want`（至多几条）、`allowed_domains`、`blocked_domains`；回应
//! `{"hits": [{"title", "url", "snippet"?, "date"?}]}`，多出来的条、写不对的条丢掉，筛、截照出厂的几家一样在联网包里做。

use std::future::Future;
use std::pin::Pin;
use std::sync::Weak;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::search_sources::{Asked, query};
use miyu_net::{Answered, Hit, Query, SearchPort, Unanswered};

/// 等扩展答多久：和出厂的几家一次请求的时限一样。
const TIMEOUT: Duration = Duration::from_secs(20);

/// 经核心问扩展。
pub(super) struct Extensions(pub(super) Weak<Core>);

impl SearchPort for Extensions {
    fn query<'a>(
        &'a self,
        (source, owner): (&'a str, Option<&'a miyu_kernel::id::AccountId>),
        asked: &'a Query,
        want: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Answered, Unanswered>> + Send + 'a>> {
        Box::pin(async move {
            let core = self
                .0
                .upgrade()
                .ok_or_else(|| Unanswered::Failed("the core is gone".to_string()))?;
            let params = json!({
                "source": source,
                "query": asked.text,
                "want": want,
                "allowed_domains": asked.allowed,
                "blocked_domains": asked.blocked,
            });
            match query(&core, (source, owner), params, TIMEOUT).await {
                Ok((name, result)) => Ok(Answered {
                    name,
                    hits: hits(&result),
                }),
                Err(Asked::Limited) => Err(Unanswered::Limited),
                Err(Asked::Failed(why)) => Err(Unanswered::Failed(why)),
            }
        })
    }
}

/// 回应里的几条：没有标题、网址的丢掉。
fn hits(result: &Value) -> Vec<Hit> {
    let text = |hit: &Value, key: &str| {
        hit.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    result["hits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|hit| {
            Some(Hit {
                title: text(hit, "title")?,
                url: text(hit, "url")?,
                snippet: text(hit, "snippet"),
                date: text(hit, "date").map(|date| date.chars().take(10).collect()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
