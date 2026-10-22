//! B 站（`net.md`「怎么走」第 12 条第 1 款，W-7 再补）：认出视频页（短链先跟完跳转再认），先调接口 `x/web-interface/view`
//! 取标题、简介、封面、UP 主、时长（施工 W-7 四补）；接口没成的整页读完，照页面脚本里 `window.__INITIAL_STATE__` 的
//! `videoData` 取，再取不到的照 `og:*` 和 `<meta name="author">`。`kind` 总是视频。
//!
//! 接口对像浏览器、又没带登录 cookie 的请求回 HTTP 412（2026-10-03 项目主人实测的就是这个），`User-Agent` 不像浏览器的
//! 照常回（2026-10-22 实测：`Miyu`、`curl/…`、不带都回 `code: 0`）；这时视频页本身却只回一页 4.6 KB、没有数据的拦截页。
//! 所以调接口用规矩里的 `api_user_agent`，读页面照旧用像浏览器的那个。接口和页面里的是同一套字段。

use reqwest::Url;
use serde_json::Value;

use super::Draft;
use crate::Kind;
use crate::Why;
use crate::fetch::Fetcher;
use crate::html;
use crate::rules::Bilibili;

/// `url` 是不是 B 站的视频页：是的交回要读的那一页（短链跟完跳转以后落到的），不是的交 `None`。
///
/// # Errors
///
/// 短链跟跳转出了错：照那个错交（过不了闸、跳转太多是 [`Why::NoPreview`]，连不上是 [`Why::Unreachable`]）。
pub(super) async fn video_page(
    fetcher: &Fetcher,
    rules: &Bilibili,
    url: &Url,
) -> Result<Option<Url>, Why> {
    let page = if rules.short_hosts.matches(url) {
        fetcher.locate(url).await?
    } else {
        url.clone()
    };
    Ok((rules.hosts.matches(&page) && is_video(&page)).then_some(page))
}

/// 视频页：路径是 `/video/BV…`（10 个字母数字）或 `/video/av…`（数字），后面可以再跟别的段。
pub(super) fn is_video(url: &Url) -> bool {
    let Some(mut segments) = url.path_segments() else {
        return false;
    };
    if segments.next() != Some("video") {
        return false;
    }
    let id = segments.next().unwrap_or_default();
    if let Some(rest) = id.strip_prefix("BV") {
        return rest.len() == 10 && rest.bytes().all(|b| b.is_ascii_alphanumeric());
    }
    id.strip_prefix("av")
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// 调接口取到的。
#[derive(Debug)]
pub(super) enum Api {
    /// 填好的卡片。
    Found(Box<Draft>),
    /// 风控拦下了：HTTP 412，或者 `code` 是 `-412`、`-352`。视频多半还在，过会儿再要常常就有了。
    Banned,
    /// 别的：连不上、回的不是 2xx、`code` 不是 0、没有标题。调的一方退回读页面。
    Missed,
}

/// 风控拦下时 `code` 是这几个（`-412` 请求被拦、`-352` 风控校验失败）。
const BANNED_CODES: [i64; 2] = [-412, -352];

/// 调接口取视频（施工 W-7 四补）：`page` 是认出来的视频页，接口的协议、端口照它，主机、路径照规矩，`bvid` 或 `aid` 照
/// 路径。
pub(super) async fn from_api(fetcher: &Fetcher, rules: &Bilibili, page: &Url) -> Api {
    let Some(url) = api(rules, page) else {
        return Api::Missed;
    };
    let answer = match fetcher.json(&url, Some(&rules.api_user_agent)).await {
        Ok(answer) => answer,
        Err(Some(412)) => return Api::Banned,
        Err(_) => return Api::Missed,
    };
    match answer.get("code").and_then(Value::as_i64) {
        Some(0) => {}
        Some(code) if BANNED_CODES.contains(&code) => return Api::Banned,
        _ => return Api::Missed,
    }
    let Some(video) = answer.get("data").and_then(read) else {
        return Api::Missed;
    };
    let mut draft = Draft {
        kind: Kind::Video,
        site_fallback: rules.site.clone(),
        ..Draft::default()
    };
    draft.found.icon = page.join("/favicon.ico").ok();
    apply(video, &mut draft);
    Api::Found(Box::new(draft))
}

/// 接口的地址：`page` 换上接口的主机、路径，查询只有 `bvid=BV…` 或者 `aid=…`。
pub(super) fn api(rules: &Bilibili, page: &Url) -> Option<Url> {
    let id = page.path_segments()?.nth(1)?.to_string();
    let mut url = page.clone();
    url.set_host(Some(&rules.api_host)).ok()?;
    url.set_path(&rules.api_path);
    url.set_fragment(None);
    let query = match id.strip_prefix("av") {
        Some(aid) => format!("aid={aid}"),
        None => format!("bvid={id}"),
    };
    url.set_query(Some(&query));
    Some(url)
}

/// 照读到的整页补上视频的几格：脚本里的 `videoData` 有的照它，没有的照页面的 `og:*`、`<meta name="author">`。
pub(super) fn fill(text: &str, rules: &Bilibili, draft: &mut Draft) {
    draft.kind = Kind::Video;
    draft.site_fallback.clone_from(&rules.site);
    let Some(video) = video_data(text).as_ref().and_then(read) else {
        draft.author = html::first(text, "meta", "name", "author", "content").unwrap_or_default();
        return;
    };
    apply(video, draft);
}

/// 把取到的视频填进卡片：封面的 `http:` 换成 `https:`（B 站给的封面地址是 `http:` 的，图床两样都给）；写了端口的照原样，
/// 换了协议端口就对不上。
fn apply(video: Video, draft: &mut Draft) {
    draft.found.title = video.title;
    if !video.description.trim().is_empty() {
        draft.found.description = video.description;
    }
    if let Some(mut picture) = video.picture.and_then(|picture| Url::parse(&picture).ok()) {
        if picture.scheme() == "http" && picture.port().is_none() {
            picture.set_scheme("https").unwrap_or(());
        }
        draft.found.image = Some(picture);
    }
    draft.author = video.author;
    draft.duration = video.duration;
}

/// 页面脚本里 `window.__INITIAL_STATE__=` 后面那个 JSON 对象的 `videoData`；没有的、读不成的交 `None`。
pub(super) fn video_data(text: &str) -> Option<Value> {
    const MARK: &str = "__INITIAL_STATE__=";
    let start = text.find(MARK)? + MARK.len();
    // 后面跟着 `;(function…` 这些脚本：只读第一个完整的 JSON 值
    let state = serde_json::Deserializer::from_str(&text[start..])
        .into_iter::<Value>()
        .next()?
        .ok()?;
    state.get("videoData").cloned()
}

/// `videoData` 里取到的一个视频（原样的字）。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Video {
    pub(super) title: String,
    pub(super) description: String,
    pub(super) picture: Option<String>,
    pub(super) author: String,
    pub(super) duration: Option<u64>,
}

/// 读 `videoData`：`title` 不空的才算一个视频。
pub(super) fn read(data: &Value) -> Option<Video> {
    let text = |pointer: &str| {
        data.pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let title = text("/title");
    if title.trim().is_empty() {
        return None;
    }
    let picture = text("/pic");
    Some(Video {
        title,
        description: text("/desc"),
        picture: (!picture.trim().is_empty()).then_some(picture),
        author: text("/owner/name"),
        duration: data
            .pointer("/duration")
            .and_then(Value::as_u64)
            .filter(|seconds| *seconds > 0),
    })
}

#[cfg(test)]
mod tests;
