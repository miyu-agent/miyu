//! `link_preview.json` 里认得的站和人机验证页那两段（`net.md`「怎么走」第 12、13 条，W-7 再补）：认哪些主机、
//! MediaWiki 的接口在哪、B 站「视频没了」的页面、抖音和 TikTok 怎么取（施工 N-6）、人机验证页长什么样。代码里只有「怎么
//! 取」，这些都是数据。

use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::Url;
use serde::Deserialize;

/// `sites` 那一段的样子。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SitesFile {
    bilibili: BilibiliFile,
    youtube: YoutubeFile,
    mediawiki: MediawikiFile,
    douyin: DouyinFile,
    tiktok: TiktokFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DouyinFile {
    hosts: Vec<String>,
    short_hosts: Vec<String>,
    site: String,
    canonical_host: String,
    register: RegisterFile,
    share: ShareFile,
    fallback: FallbackFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterFile {
    host: String,
    path: String,
    user_agent: String,
    body: serde_json::Value,
    keep_hours: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShareFile {
    host: String,
    user_agent: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FallbackFile {
    user_agent: String,
    title_suffix: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TiktokFile {
    hosts: Vec<String>,
    short_hosts: Vec<String>,
    site: String,
    user_agent: String,
    accept_language: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BilibiliFile {
    hosts: Vec<String>,
    short_hosts: Vec<String>,
    site: String,
    api_host: String,
    api_path: String,
    api_user_agent: String,
    gone_titles: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct YoutubeFile {
    hosts: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MediawikiFile {
    hosts: BTreeMap<String, String>,
    thumbnail: u32,
}

/// `challenge` 那一段的样子。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ChallengeFile {
    titles: Vec<String>,
    headers: BTreeMap<String, String>,
    statuses: Vec<u16>,
}

/// 一张主机的表：主机和列着的一样，或者以「`.` 加列着的」结尾，就算（第 12 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hosts(Vec<String>);

impl Hosts {
    /// `url` 的主机在不在表里。
    pub(crate) fn matches(&self, url: &Url) -> bool {
        self.0.iter().any(|entry| host_matches(url, entry))
    }
}

/// `url` 的主机（小写、去掉末尾的点）是不是 `entry`，或者是它的子域名。`notbilibili.com` 不算 `bilibili.com`。
fn host_matches(url: &Url, entry: &str) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == entry
        || host
            .strip_suffix(entry)
            .is_some_and(|head| head.ends_with('.'))
}

/// B 站的规矩。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bilibili {
    /// 视频页在哪些主机上。
    pub(crate) hosts: Hosts,
    /// 短链的主机：先跟完跳转再认。
    pub(crate) short_hosts: Hosts,
    /// 站名的退路：页面没写 `og:site_name` 时用它。
    pub(crate) site: String,
    /// 视频接口的主机、路径（施工 W-7 四补）：协议、端口照视频页的。
    pub(crate) api_host: String,
    pub(crate) api_path: String,
    /// 调接口用的 `User-Agent`：不像浏览器的才给（像浏览器、又没带登录的 cookie 的回 412）。
    pub(crate) api_user_agent: String,
    /// 页面标题是这些的（收拢空白以后）：B 站「视频没了」的那一页。
    pub(crate) gone_titles: Vec<String>,
}

/// MediaWiki 站的规矩。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mediawiki {
    /// 列着的主机和它的 `api.php` 的路径。
    hosts: Vec<(String, String)>,
    /// 要的缩略图多宽。
    pub(crate) thumbnail: u32,
}

impl Mediawiki {
    /// `url` 的主机列着的话，它的 `api.php` 的路径。
    pub(crate) fn api_path(&self, url: &Url) -> Option<&str> {
        self.hosts
            .iter()
            .find(|(entry, _)| host_matches(url, entry))
            .map(|(_, path)| path.as_str())
    }
}

/// 抖音的规矩（施工 N-6）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Douyin {
    /// 视频页在哪些主机上（网页版、手机分享页）。
    pub(crate) hosts: Hosts,
    /// 短链的主机：先跟完跳转再认。
    pub(crate) short_hosts: Hosts,
    /// 站名。
    pub(crate) site: String,
    /// 卡片的地址、兜底页、图标照这个主机（`www.douyin.com`）。
    pub(crate) canonical_host: String,
    /// 拿匿名 `ttwid` 的地方、`User-Agent`、请求体（JSON 原文），拿到的记多久。
    pub(crate) register_host: String,
    pub(crate) register_path: String,
    pub(crate) register_user_agent: String,
    pub(crate) register_body: String,
    pub(crate) keep: Duration,
    /// 手机分享页的主机、`User-Agent`。
    pub(crate) share_host: String,
    pub(crate) share_user_agent: String,
    /// 兜底读网页版的 `User-Agent`（搜索引擎的爬虫拿得到整页），标题末尾去掉的那一截。
    pub(crate) fallback_user_agent: String,
    pub(crate) title_suffix: String,
}

/// TikTok 的规矩（施工 N-6）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tiktok {
    /// 视频页在哪些主机上。
    pub(crate) hosts: Hosts,
    /// 短链的主机：先跟完跳转再认。
    pub(crate) short_hosts: Hosts,
    /// 站名。
    pub(crate) site: String,
    /// 读页面的 `User-Agent`、`Accept-Language`。
    pub(crate) user_agent: String,
    pub(crate) accept_language: String,
}

/// 认得的站。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sites {
    /// B 站。
    pub(crate) bilibili: Bilibili,
    /// YouTube 在哪些主机上。
    pub(crate) youtube: Hosts,
    /// MediaWiki 站。
    pub(crate) mediawiki: Mediawiki,
    /// 抖音（施工 N-6）。
    pub(crate) douyin: Douyin,
    /// TikTok（施工 N-6）。
    pub(crate) tiktok: Tiktok,
}

/// 人机验证、挡爬虫的页面怎么认（第 13 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Challenge {
    /// 标题完全对上这些的（收拢空白以后）。
    pub(crate) titles: Vec<String>,
    /// 带着这些头、值也对上的（名字小写；值去掉两头空白、不分大小写比）。
    pub(crate) headers: Vec<(String, String)>,
    /// 回这些状态码、又是 HTML 的。
    pub(crate) statuses: Vec<u16>,
}

/// 读好 `sites`：主机一律小写。
pub(super) fn sites(file: SitesFile) -> Result<Sites, String> {
    if file.mediawiki.thumbnail == 0 {
        return Err("sites.mediawiki.thumbnail 不能是 0".to_string());
    }
    if file.douyin.register.keep_hours == 0 {
        return Err("sites.douyin.register.keep_hours 不能是 0".to_string());
    }
    let douyin = file.douyin;
    let tiktok = file.tiktok;
    Ok(Sites {
        bilibili: Bilibili {
            hosts: hosts(file.bilibili.hosts),
            short_hosts: hosts(file.bilibili.short_hosts),
            site: file.bilibili.site,
            api_host: file.bilibili.api_host.to_ascii_lowercase(),
            api_path: file.bilibili.api_path,
            api_user_agent: file.bilibili.api_user_agent,
            gone_titles: file.bilibili.gone_titles,
        },
        youtube: hosts(file.youtube.hosts),
        mediawiki: Mediawiki {
            hosts: file
                .mediawiki
                .hosts
                .into_iter()
                .map(|(host, path)| (host.to_ascii_lowercase(), path))
                .collect(),
            thumbnail: file.mediawiki.thumbnail,
        },
        douyin: Douyin {
            hosts: hosts(douyin.hosts),
            short_hosts: hosts(douyin.short_hosts),
            site: douyin.site,
            canonical_host: douyin.canonical_host.to_ascii_lowercase(),
            register_host: douyin.register.host.to_ascii_lowercase(),
            register_path: douyin.register.path,
            register_user_agent: douyin.register.user_agent,
            register_body: douyin.register.body.to_string(),
            keep: Duration::from_secs(douyin.register.keep_hours.saturating_mul(3600)),
            share_host: douyin.share.host.to_ascii_lowercase(),
            share_user_agent: douyin.share.user_agent,
            fallback_user_agent: douyin.fallback.user_agent,
            title_suffix: douyin.fallback.title_suffix,
        },
        tiktok: Tiktok {
            hosts: hosts(tiktok.hosts),
            short_hosts: hosts(tiktok.short_hosts),
            site: tiktok.site,
            user_agent: tiktok.user_agent,
            accept_language: tiktok.accept_language,
        },
    })
}

/// 读好 `challenge`：头的名字一律小写。
pub(super) fn challenge(file: ChallengeFile) -> Challenge {
    Challenge {
        titles: file.titles,
        headers: file
            .headers
            .into_iter()
            .map(|(name, value)| (name.to_ascii_lowercase(), value))
            .collect(),
        statuses: file.statuses,
    }
}

fn hosts(list: Vec<String>) -> Hosts {
    Hosts(
        list.into_iter()
            .map(|host| host.to_ascii_lowercase())
            .collect(),
    )
}
