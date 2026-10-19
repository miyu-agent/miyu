//! 自己抓不下来的请别家代抓（施工 N-4，设计 36 的 N-4）：被挡（人机验证、401、403、429、451、503）、连不上、抓回来一个字
//! 都没有、短得像登录墙的，交给 [`Search::fetch`] 照 `search.json` 的 `fetch` 一家家试；代抓到的记进 [`Pages`]，翻页不再
//! 去抓。都不成的照自己抓的那个结果交：她看到的是原来的原因。不是网址、内网、跳转太多、读不成字的种类不代抓。

use std::sync::Arc;

use crate::document::{Miss, Pages, Read};
use crate::search::{Search, Using};

/// 自己抓的结果 `own` 值不值得请别家试。
pub(super) fn worth(own: &Result<Arc<Read>, Miss>, search: &Search) -> bool {
    match own {
        Ok(read) => read.via.is_none() && search.too_short(&read.text),
        Err(Miss::Challenge | Miss::Empty | Miss::Unreachable) => true,
        Err(Miss::Status(status)) => matches!(status, 401 | 403 | 429 | 451 | 503),
        Err(_) => false,
    }
}

/// 请别家代抓网址 `url`：抓到的记进 `pages`、交回；都不成的交回自己抓的 `own`。
pub(super) async fn rescue(
    own: Result<Arc<Read>, Miss>,
    url: &str,
    (pages, search, using): (&Pages, &Search, &Using),
) -> Result<Arc<Read>, Miss> {
    let Ok(fetched) = search.fetch(url.trim(), using).await else {
        return own;
    };
    let read = Arc::new(Read {
        url: url.trim().to_string(),
        title: fetched.title,
        text: fetched.text,
        cut: fetched.cut,
        via: Some(fetched.source),
    });
    pages.keep(url, Arc::clone(&read));
    Ok(read)
}
