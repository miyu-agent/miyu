//! 抖音（施工 N-6，`net.md`「怎么走」第 12 条）：认出作品页（短链先跟完跳转再认），主路带匿名 `ttwid` 读手机分享页脚本里的
//! `window._ROUTER_DATA`，取不到的兜底照搜索引擎爬虫读网页版的 JSON-LD。`kind` 总是视频。2026-10-24 终端实测、录了页面。
//!
//! - 匿名 `ttwid`：照规矩 `POST` 注册，拿回应头里的 cookie，记 `keep` 那么久，一个核心共用一份。不带它，分享页照样回
//!   200，只是里面没有作品数据。
//! - 分享页里 `item_list` 空、`filter_list` 不空的：删了、只自己可见，当不可达。都空、没有作品数据的走兜底。
//! - 兜底的网页版删了的回 404，当不可达；页面里没有 `VideoObject` 的（改版、拦了）退回普通网页。
//! - 接口、兜底页的协议、端口照认出来的那一页（同 B 站的接口）。卡片的地址、图标照网页版的主机。

use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use reqwest::Url;
use serde_json::Value;

use super::Draft;
use crate::body::Until;
use crate::fetch::{Extra, Fetcher};
use crate::rules::Douyin;
use crate::{Kind, Why};

/// 一个作品：编号、是不是图文、认出来的那一页（短链跟完以后的）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Work {
    pub(super) id: String,
    pub(super) note: bool,
    pub(super) page: Url,
}

/// 读到的。
#[derive(Debug)]
pub(super) enum Read {
    /// 填好的卡片和它的地址（网页版的作品页）。
    Found(Url, Box<Draft>),
    /// 删了、只自己可见：当不可达。
    Gone,
    /// 都没取到：退回普通网页。
    Missed,
}

/// 记着的匿名 `ttwid`（值和拿到的时刻）。一个核心一份。
#[derive(Debug, Default)]
pub(crate) struct Ttwid(Mutex<Option<(String, Instant)>>);

/// `url` 是不是抖音的作品页：短链先跟完跳转。是的交回作品，不是的（落到首页的）交 `None`，交回落到的那一页。
///
/// # Errors
///
/// 短链跟跳转出了错：照那个错交。
pub(super) async fn work(
    fetcher: &Fetcher,
    rules: &Douyin,
    url: &Url,
) -> Result<Result<Work, Url>, Why> {
    let page = if rules.short_hosts.matches(url) {
        fetcher.locate(url).await?
    } else {
        url.clone()
    };
    Ok(match id_of(rules, &page) {
        Some((id, note)) => Ok(Work { id, note, page }),
        None => Err(page),
    })
}

/// 作品编号和是不是图文：`/video/<id>`、`/note/<id>`、`/share/video/<id>/`、`/share/note/<id>/`，`<id>` 是数字。
pub(super) fn id_of(rules: &Douyin, url: &Url) -> Option<(String, bool)> {
    if !rules.hosts.matches(url) || rules.short_hosts.matches(url) {
        return None;
    }
    let segments: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    let (kind, id) = match segments.as_slice() {
        ["share", kind, id] | [kind, id] => (*kind, *id),
        _ => return None,
    };
    let note = match kind {
        "video" => false,
        "note" => true,
        _ => return None,
    };
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())).then(|| (id.to_string(), note))
}

/// 照主路、兜底取一个作品。
pub(super) async fn read(fetcher: &Fetcher, rules: &Douyin, ttwid: &Ttwid, work: &Work) -> Read {
    let Some(canonical) = at(&work.page, &rules.canonical_host, &path(work, false)) else {
        return Read::Missed;
    };
    if let Some(cookie) = ttwid.get(fetcher, rules, &work.page).await {
        match share(fetcher, rules, work, &cookie).await {
            Shared::Found(draft) => {
                let draft = finish(*draft, &canonical, rules);
                return Read::Found(canonical, Box::new(draft));
            }
            Shared::Gone => return Read::Gone,
            Shared::Missed => super::fell_back("douyin share"),
        }
    }
    match fetcher
        .page_with(
            &canonical,
            Until::End,
            &rules.fallback_user_agent,
            Extra::default(),
        )
        .await
    {
        Ok((_, text)) => match video_object(&text, &rules.title_suffix) {
            Some(draft) => Read::Found(
                canonical.clone(),
                Box::new(finish(draft, &canonical, rules)),
            ),
            None => Read::Missed,
        },
        Err(Why::Unreachable) => Read::Gone,
        Err(_) => Read::Missed,
    }
}

impl Ttwid {
    /// 记着的、没过期的照它；没有的照规矩注册一个（协议、端口照 `page`），拿不到的这一次没有、下一次再拿。
    async fn get(&self, fetcher: &Fetcher, rules: &Douyin, page: &Url) -> Option<String> {
        let now = Instant::now();
        let kept = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .filter(|(_, at)| now.duration_since(*at) < rules.keep);
        if let Some((value, _)) = kept {
            return Some(value);
        }
        let url = at(page, &rules.register_host, &rules.register_path)?;
        let value = fetcher
            .cookie_from(
                &url,
                &rules.register_user_agent,
                &rules.register_body,
                "ttwid",
            )
            .await?;
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some((value.clone(), now));
        Some(value)
    }
}

/// 分享页读到的。
#[derive(Debug)]
pub(super) enum Shared {
    Found(Box<Draft>),
    Gone,
    Missed,
}

/// 带着 `ttwid` 读手机分享页。
async fn share(fetcher: &Fetcher, rules: &Douyin, work: &Work, ttwid: &str) -> Shared {
    let Some(url) = at(&work.page, &rules.share_host, &path(work, true)) else {
        return Shared::Missed;
    };
    let cookie = format!("ttwid={ttwid}");
    let extra = Extra {
        cookie: Some(&cookie),
        language: None,
    };
    let Ok((_, text)) = fetcher
        .page_with(&url, Until::End, &rules.share_user_agent, extra)
        .await
    else {
        return Shared::Missed;
    };
    shared(&text)
}

/// 照分享页的字认：第一件作品读得出的是它；没有作品、`filter_list` 不空的是删了；别的没取到。
pub(super) fn shared(text: &str) -> Shared {
    let Some((items, nothing_filtered)) = router_data(text).as_ref().and_then(lists) else {
        return Shared::Missed;
    };
    match items.first() {
        Some(item) => {
            item_draft(item).map_or(Shared::Missed, |draft| Shared::Found(Box::new(draft)))
        }
        None if !nothing_filtered => Shared::Gone,
        None => Shared::Missed,
    }
}

/// 页面脚本里 `window._ROUTER_DATA = ` 后面的第一个完整的 JSON 值；没有的、读不成的交 `None`。
pub(super) fn router_data(text: &str) -> Option<Value> {
    const MARK: &str = "_ROUTER_DATA";
    let start = text.find(MARK)? + MARK.len();
    let rest = text[start..].trim_start().strip_prefix('=')?;
    serde_json::Deserializer::from_str(rest)
        .into_iter::<Value>()
        .next()?
        .ok()
}

/// `loaderData` 里带 `videoInfoRes.item_list` 的那一个：作品列表，和 `filter_list` 是不是空的（没写的也算空）。
fn lists(data: &Value) -> Option<(Vec<Value>, bool)> {
    data.get("loaderData")?
        .as_object()?
        .values()
        .find_map(|value| {
            let res = value.get("videoInfoRes")?;
            let items = res.get("item_list")?.as_array()?.clone();
            let nothing_filtered = res
                .get("filter_list")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty);
            Some((items, nothing_filtered))
        })
}

/// 一件作品：`desc` 当标题（空的照作者名），时长毫秒四舍五入成秒，封面是 `video.cover.url_list[0]`。
pub(super) fn item_draft(item: &Value) -> Option<Draft> {
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
            .map(|ms| ms.saturating_add(500) / 1000)
            .filter(|seconds| *seconds > 0),
        ..Draft::default()
    };
    draft.found.title = title;
    draft.found.image = item
        .pointer("/video/cover/url_list/0")
        .and_then(Value::as_str)
        .and_then(|cover| Url::parse(cover).ok());
    Some(draft)
}

/// 网页版页面里 `@type` 是 `VideoObject` 的那段 JSON-LD：`name` 去掉 `suffix` 当标题，`thumbnailUrl` 第一个当封面，
/// `duration`（ISO 8601）换成秒，`creator.name` 作者。`description` 是套话，不用。
pub(super) fn video_object(text: &str, suffix: &str) -> Option<Draft> {
    const MARK: &str = "application/ld+json";
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        rest = &rest[at + MARK.len()..];
        let Some(open) = rest.find('>') else { break };
        let body = &rest[open + 1..];
        let Some(close) = body.find("</script>") else {
            break;
        };
        let Ok(value) = serde_json::from_str::<Value>(&body[..close]) else {
            continue;
        };
        if value.get("@type").and_then(Value::as_str) != Some("VideoObject") {
            continue;
        }
        let name = value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let title = name.strip_suffix(suffix).unwrap_or(name).trim().to_string();
        let author = value
            .pointer("/creator/name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let title = if title.is_empty() {
            author.clone()
        } else {
            title
        };
        if title.is_empty() {
            return None;
        }
        let thumbnail = match value.get("thumbnailUrl") {
            Some(Value::Array(list)) => list.first().and_then(Value::as_str),
            Some(Value::String(one)) => Some(one.as_str()),
            _ => None,
        };
        let mut draft = Draft {
            kind: Kind::Video,
            author,
            duration: value
                .get("duration")
                .and_then(Value::as_str)
                .and_then(iso_seconds)
                .filter(|seconds| *seconds > 0),
            ..Draft::default()
        };
        draft.found.title = title;
        draft.found.image = thumbnail.and_then(|url| Url::parse(url).ok());
        return Some(draft);
    }
    None
}

/// ISO 8601 的时长（`PT0H0M15S`、`PT1M`、`P1DT2H`）换成秒；写法不对的交 `None`。小数部分舍掉。
pub(super) fn iso_seconds(text: &str) -> Option<u64> {
    let rest = text.strip_prefix('P')?;
    let mut seconds: u64 = 0;
    let mut number = String::new();
    let mut time = false;
    for c in rest.chars() {
        match c {
            '0'..='9' | '.' => number.push(c),
            'T' if number.is_empty() => time = true,
            unit => {
                let whole: u64 = number.split('.').next()?.parse().ok()?;
                number.clear();
                let scale = match (time, unit) {
                    (false, 'D') => 86_400,
                    (true, 'H') => 3_600,
                    (true, 'M') => 60,
                    (true, 'S') => 1,
                    _ => return None,
                };
                seconds = seconds.checked_add(whole.checked_mul(scale)?)?;
            }
        }
    }
    number.is_empty().then_some(seconds)
}

/// 卡片的站名、图标照网页版的主机。
fn finish(mut draft: Draft, canonical: &Url, rules: &Douyin) -> Draft {
    draft.site_fallback.clone_from(&rules.site);
    draft.found.icon = canonical.join("/favicon.ico").ok();
    draft
}

/// 作品页的路径：网页版 `/video/<id>`，分享页 `/share/video/<id>/`（图文换成 `note`）。
fn path(work: &Work, share: bool) -> String {
    let kind = if work.note { "note" } else { "video" };
    match share {
        true => format!("/share/{kind}/{}/", work.id),
        false => format!("/{kind}/{}", work.id),
    }
}

/// `page` 换上主机 `host`、路径 `path`，去掉查询和片段：协议、端口照 `page`。
fn at(page: &Url, host: &str, path: &str) -> Option<Url> {
    let mut url = page.clone();
    url.set_host(Some(host)).ok()?;
    url.set_path(path);
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

#[cfg(test)]
mod tests;
