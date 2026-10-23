//! TikTok（施工 N-6，`net.md`「怎么走」第 12 条）：认出视频页（短链先跟完跳转再认），整页读完，照页面脚本
//! `__UNIVERSAL_DATA_FOR_REHYDRATION__` 里 `webapp.video-detail` 的 `itemStruct` 取。`kind` 总是视频。2026-10-24 终端实测、
//! 录了页面：页面没有 `og:`、没有 JSON-LD，数据只在这段脚本里。
//!
//! 拿不到这段脚本、没有 `itemStruct`、`statusCode` 不是 0（`10204` 是不存在）的都当不可达：TikTok 有 WAF，拦下的页面
//! 没这段数据，过会儿再要可能就有了。

use reqwest::Url;
use serde_json::Value;

use super::Draft;
use crate::body::Until;
use crate::fetch::{Extra, Fetcher};
use crate::rules::Tiktok;
use crate::{Kind, Why};

/// `url` 是不是 TikTok 的视频页：短链先跟完跳转。是的交回要读的那一页（去掉查询、片段），不是的交回落到的那一页。
///
/// # Errors
///
/// 短链跟跳转出了错：照那个错交。
pub(super) async fn video_page(
    fetcher: &Fetcher,
    rules: &Tiktok,
    url: &Url,
) -> Result<Result<Url, Url>, Why> {
    let page = if rules.short_hosts.matches(url) {
        fetcher.locate(url).await?
    } else {
        url.clone()
    };
    if !is_video(rules, &page) {
        return Ok(Err(page));
    }
    let mut page = page;
    page.set_query(None);
    page.set_fragment(None);
    Ok(Ok(page))
}

/// 视频页：路径是 `/@<用户>/video/<数字>`，用户名可以是空的（短链跳过去就是 `@/video/<id>`）。
pub(super) fn is_video(rules: &Tiktok, url: &Url) -> bool {
    if !rules.hosts.matches(url) || rules.short_hosts.matches(url) {
        return false;
    }
    let Some(segments) = url.path_segments() else {
        return false;
    };
    let segments: Vec<&str> = segments.filter(|s| !s.is_empty()).collect();
    matches!(segments.as_slice(), [user, "video", id]
        if user.starts_with('@') && !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
}

/// 读视频页取卡片。
///
/// # Errors
///
/// 读不到页面、没有那段脚本、没有 `itemStruct`、`statusCode` 不是 0：[`Why::Unreachable`]。
pub(super) async fn read(fetcher: &Fetcher, rules: &Tiktok, page: &Url) -> Result<Draft, Why> {
    let extra = Extra {
        cookie: None,
        language: Some(&rules.accept_language),
    };
    let (_, text) = fetcher
        .page_with(page, Until::End, &rules.user_agent, extra)
        .await
        .map_err(|_| Why::Unreachable)?;
    let mut draft = item(&text).ok_or(Why::Unreachable)?;
    draft.site_fallback.clone_from(&rules.site);
    draft.found.icon = page.join("/favicon.ico").ok();
    Ok(draft)
}

/// 照页面的字取：`webapp.video-detail` 的 `statusCode` 是 0、有 `itemStruct` 的才算。`desc` 当标题（空的照作者名），
/// `video.duration` 秒，`video.cover` 封面。
pub(super) fn item(text: &str) -> Option<Draft> {
    let data = universal_data(text)?;
    let detail = data.get("__DEFAULT_SCOPE__")?.get("webapp.video-detail")?;
    if detail.get("statusCode").and_then(Value::as_i64) != Some(0) {
        return None;
    }
    let item = detail.pointer("/itemInfo/itemStruct")?;
    let text = |pointer: &str| {
        item.pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let author = text("/author/nickname");
    let title = Some(text("/desc"))
        .filter(|desc| !desc.is_empty())
        .unwrap_or_else(|| author.clone());
    if title.is_empty() {
        return None;
    }
    let mut draft = Draft {
        kind: Kind::Video,
        author,
        duration: item
            .pointer("/video/duration")
            .and_then(Value::as_u64)
            .filter(|seconds| *seconds > 0),
        ..Draft::default()
    };
    draft.found.title = title;
    draft.found.image = Url::parse(&text("/video/cover")).ok();
    Some(draft)
}

/// `<script id="__UNIVERSAL_DATA_FOR_REHYDRATION__" …>` 到它的 `</script>` 那段 JSON；没有的、读不成的交 `None`。
pub(super) fn universal_data(text: &str) -> Option<Value> {
    const MARK: &str = "id=\"__UNIVERSAL_DATA_FOR_REHYDRATION__\"";
    let start = text.find(MARK)?;
    let body = &text[start..];
    let body = &body[body.find('>')? + 1..];
    let end = body.find("</script>")?;
    serde_json::from_str(&body[..end]).ok()
}

#[cfg(test)]
mod tests;
