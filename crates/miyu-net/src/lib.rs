//! 可选软件包 `net`（设计 10 第四节，`net.md`，施工 W-7）：核心去抓别人家网页的地方。链接卡片
//! [`LinkPreview::preview`] 抓下一页的标题、简介、图，图存成数据根的 blob；她的工具 `web_fetch`（施工 N-1，设计 36，
//! `web_fetch.rs`、`document.rs`）抓一份网页给她读。两边同一道地址闸、同一套代理。
//!
//! - 地址闸（`guard.rs`）是安全边界：每一跳都过，不走代理的钉住解析好的地址（`fetch.rs`）。
//! - 代理照环境变量，和请求模型同一套读法（`proxy.rs`）。
//! - 第一次调才读 `link_preview.json`、读环境变量里的代理；读不懂的记一条 `WARN not ready`，这个核心的生命周期里
//!   不再试（照 `mermaid.md` 第 2 条）。
//! - 抓过的记在内存里（`remember.rs`）；记着的卡片指的 blob 没了的，那一格交 `None`。
//! - 测试的口子在 `testkit.rs`，只在 `testkit` 开关打开时编进去。
//! - 认得的站（B 站、YouTube、MediaWiki 站）照各自的办法取，人机验证页不出卡片（`sites.rs`，W-7 再补）。

mod body;
mod document;
mod fetch;
mod html;
mod remember;
mod rules;
mod search;
mod settings;
mod sites;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
mod web_fetch;
mod web_search;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use miyu_kernel::id::ContentHash;
use miyu_store::blob::Blobs;
use reqwest::Url;

use fetch::Fetcher;
use miyu_http::guard::Guard;
use miyu_http::proxy::Proxies;
use remember::{Keep, Remember};
use rules::{Clip, Sites};

pub use document::{Miss, Pages, Read};
pub use search::{
    Answered, Fetched, Found, Hit, Missed, Query, Search, SearchPort, Unanswered, Using,
};
pub use settings::{
    AUTO, BraveKey, DeepseekKey, ExaKey, ITEMS, Missing, SearchSettings, SearxngAddress, TavilyKey,
    missing, with_extension_sources,
};
pub use web_fetch::{PACKAGE, WEB_FETCH};
pub use web_search::WEB_SEARCH;

use miyu_tool::Tool;
use miyu_tool::load::LoadError;

/// 她的工具：`web_fetch`、`web_search`。字从资源目录 `resources` 读，抓网页、搜网页照出厂的规矩。冷却、记着的页记在这一份
/// 家底里：装卸软件包以后核心重新要工具，从头算。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对。
pub fn tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    tools_with(
        resources,
        Arc::new(Pages::new(resources)),
        Arc::new(Search::new(resources)),
    )
}

/// 同 [`tools`]，抓网页照给的 `pages`、搜网页照给的 `search`：核心起来时造一份，装卸软件包重新要工具时照旧用它，冷却、记着
/// 的页不从头算（施工 N-2 下）；测试照 `Pages::testing`、`Search::testing` 打开口子。
///
/// # Errors
///
/// 同 [`tools`]。
pub fn tools_with(
    resources: &Path,
    pages: Arc<Pages>,
    search: Arc<Search>,
) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    Ok(vec![
        Arc::new(web_fetch::WebFetch::load(
            resources,
            pages,
            Arc::clone(&search),
        )?),
        Arc::new(web_search::WebSearch::load(resources, search)?),
    ])
}

/// 运行日志的目标。
const TARGET: &str = "miyu::net";

/// 谁要她抓网页（施工 MB-2，项目主人 2026-10-22 定）：都过同一道闸。别人的
/// （场所里的人）另有两条：配置里的内部域名一律不去；本机解析不出来、要交给代理的，主机名是一个词的、带内网后缀的
/// （规矩文件的 `internal_suffixes`）不交，代理那头可能解析得出内网的名字，别的照旧交（被墙的境外站照样抓得到）。抓过的照谁要的
/// 分开记，主人抓到的不给别人。链接卡片（`link.preview`）只有主人调得了，不分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asker {
    /// 主人：主人自己在本机的会话、自己的连接。
    Owner,
    /// 别人：场所会话里的人，拿不准的也算。
    Other {
        /// 配置里的内部域名（`net.internal_domains`）：这些域名和它们的子域名一律不去。
        internal: Vec<String>,
    },
}

impl Asker {
    /// 替别人抓、主机名 `host` 是内部域名或者它的子域名：不去。机器主人的都不算。大小写、首尾的点不论。
    pub(crate) fn internal(&self, host: &str) -> bool {
        let Asker::Other { internal } = self else {
            return false;
        };
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        internal.iter().any(|domain| {
            let domain = domain.trim_matches('.').to_ascii_lowercase();
            !domain.is_empty() && fetch::within(&host, &domain)
        })
    }
}

/// 一张卡片。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// 跟完跳转、最后落到的那一页。
    pub url: String,
    /// 标题：收拢了空白、截好了。
    pub title: String,
    /// 简介，可以是空的。
    pub description: String,
    /// 站名：`og:site_name`，没有的是主机名去掉 `www.`。
    pub site: String,
    /// 卡片的图。
    pub image: Option<Picture>,
    /// 站点的图标。
    pub icon: Option<Picture>,
    /// 是什么（W-7 再补）。
    pub kind: Kind,
    /// 视频多少秒：B 站接口给的、YouTube 页面上读到的；没有的是 `None`。
    pub duration: Option<u64>,
    /// 作者：B 站的 UP 主、YouTube 的频道名；没有的是 `None`。
    pub author: Option<String>,
}

/// 卡片是什么（`net.md`「对外的样子」，W-7 再补）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Kind {
    /// 一个视频：B 站的视频、YouTube 上读到了时长的页。
    Video,
    /// 一篇文章：MediaWiki 站上的页。
    Article,
    /// 别的网页。
    #[default]
    Page,
}

impl Kind {
    /// 协议上的写法：`video`、`article`、`page`。
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Video => "video",
            Kind::Article => "article",
            Kind::Page => "page",
        }
    }
}

/// 卡片上的一张图：数据根的 blob。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// 内容哈希：头照 `blob.get` 读。
    pub blob: ContentHash,
    /// 照开头的魔数认出来的类型：`image/png`、`image/jpeg`、`image/gif`、`image/webp`、`image/x-icon`。
    pub media_type: &'static str,
}

/// 做不出卡片的四种（`net.md`「对外的样子」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// 读不成地址。
    NotAUrl,
    /// 不是 http、https。
    UnsupportedScheme,
    /// 不是网页、没有标题、地址过不了闸、跳转太多：下次也一样。
    NoPreview,
    /// 超时、连不上、对方回 4xx、5xx：过会儿可能就好了。
    Unreachable,
}

impl Why {
    /// 协议上的写法：`not_a_url`、`unsupported_scheme`、`no_preview`、`unreachable`。
    pub fn as_str(self) -> &'static str {
        match self {
            Why::NotAUrl => "not_a_url",
            Why::UnsupportedScheme => "unsupported_scheme",
            Why::NoPreview => "no_preview",
            Why::Unreachable => "unreachable",
        }
    }
}

/// 一次 [`LinkPreview::preview`] 的结果：做不出卡片是正常的结果之一，不是出错。
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::large_enum_variant,
    reason = "一次 preview 交回一个，马上写成回应；记着的放的是 Card，不是它"
)]
pub enum Preview {
    /// 做成了。
    Card(Card),
    /// 没做成，为什么。
    Miss(Why),
}

/// 没法抓：`link_preview.json` 读不懂。原话记运行日志，协议上是 `internal_error`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotReady;

/// 第一次调时备好的：抓取、截断、记着的。
struct Ready {
    fetcher: Fetcher,
    sites: Sites,
    /// 抖音的匿名 `ttwid`（施工 N-6）：记一天，一个核心共用一份。
    ttwid: sites::Ttwid,
    challenge_titles: Vec<String>,
    clip: Clip,
    remember: Remember,
}

/// 链接卡片的家底：资源目录在哪、图存进哪份 blob、懒备好的抓取。一个核心一份，一直留到核心退出。
pub struct LinkPreview {
    resources: PathBuf,
    blobs: Blobs,
    ready: OnceLock<Result<Ready, ()>>,
    #[cfg(any(test, feature = "testkit"))]
    testing: Option<testkit::Testing>,
}

impl LinkPreview {
    /// 一份家底：只记资源目录在哪、图存进 `blobs`，不碰磁盘、不读环境变量。真正的初始化等第一次
    /// [`LinkPreview::preview`]。
    pub fn new(resources: &Path, blobs: Blobs) -> LinkPreview {
        LinkPreview {
            resources: resources.to_path_buf(),
            blobs,
            ready: OnceLock::new(),
            #[cfg(any(test, feature = "testkit"))]
            testing: None,
        }
    }

    /// 测试用：闸和代理照 `testing`，不照平时的（`net.md`「起草时定的」第 3 条）。生产的构建里没有这个方法。
    #[cfg(any(test, feature = "testkit"))]
    #[must_use]
    pub fn testing(mut self, testing: testkit::Testing) -> LinkPreview {
        self.testing = Some(testing);
        self
    }

    /// 一个链接的卡片（`net.md`「怎么走」）：去掉前后空白读成网址，过闸、抓页面、挖元数据、抓图存成 blob。抓过的
    /// 照记着的给；记着的卡片指的 blob 没了的，那一格交 `None`。
    ///
    /// # Errors
    ///
    /// `link_preview.json` 读不懂（[`NotReady`]，原话记运行日志）。
    pub async fn preview(&self, text: &str) -> Result<Preview, NotReady> {
        let Ok(url) = Url::parse(text.trim()) else {
            return Ok(Preview::Miss(Why::NotAUrl));
        };
        if !matches!(url.scheme(), "http" | "https") {
            return Ok(Preview::Miss(Why::UnsupportedScheme));
        }
        let ready = self.ready()?;
        let key = url.as_str().to_string();
        let outcome = match ready.remember.get(&key, Instant::now()) {
            Some(hit) => hit,
            None => {
                let outcome = self.fetch(ready, &url).await;
                if let Err(why) = &outcome {
                    tracing::warn!(target: TARGET, why = why.as_str(), "link preview failed");
                }
                ready.remember.put(key, outcome.clone(), Instant::now());
                outcome
            }
        };
        Ok(match outcome {
            Ok(card) => Preview::Card(self.still_there(card).await),
            Err(why) => Preview::Miss(why),
        })
    }

    /// 抓一页做成卡片（认得的站照它的办法）：一张卡至少要有个标题，不然不如留着原来的链接。
    async fn fetch(&self, ready: &Ready, url: &Url) -> Result<Card, Why> {
        let (page, draft) = sites::draft(
            &ready.fetcher,
            (&ready.sites, &ready.ttwid),
            &ready.challenge_titles,
            url,
        )
        .await?;
        let texts = sites::finish(&draft, &page, &ready.clip)?;
        // 两张图互不相干，一起抓
        let (image, icon) = tokio::join!(
            self.keep(ready, draft.found.image.as_ref()),
            self.keep(ready, draft.found.icon.as_ref())
        );
        Ok(Card {
            url: page.to_string(),
            title: texts.title,
            description: texts.description,
            site: texts.site,
            image,
            icon,
            kind: draft.kind,
            duration: draft.duration,
            author: texts.author,
        })
    }

    /// 抓一张图存成 blob；抓不到、认不出、存不进的没有（卡片照样成立）。存在阻塞线程里。
    async fn keep(&self, ready: &Ready, url: Option<&Url>) -> Option<Picture> {
        let (bytes, media_type) = ready.fetcher.image(url?).await?;
        let blobs = self.blobs.clone();
        let stored = tokio::task::spawn_blocking(move || blobs.put(&bytes)).await;
        match stored {
            Ok(Ok(blob)) => Some(Picture { blob, media_type }),
            Ok(Err(error)) => {
                tracing::warn!(target: TARGET, error = %error, "link image not stored");
                None
            }
            Err(error) => {
                tracing::warn!(target: TARGET, panic = error.is_panic(), "link image not stored");
                None
            }
        }
    }

    /// 卡片上的图还在不在：blob 没了的那一格换成 `None`（第 9 条）。看文件在阻塞线程里。
    async fn still_there(&self, mut card: Card) -> Card {
        let blobs = self.blobs.clone();
        let (image, icon) = (card.image.take(), card.icon.take());
        let checked = tokio::task::spawn_blocking(move || {
            let kept = |picture: Option<Picture>| {
                picture.filter(|picture| blobs.path(&picture.blob).is_file())
            };
            (kept(image), kept(icon))
        })
        .await;
        if let Ok((image, icon)) = checked {
            card.image = image;
            card.icon = icon;
        }
        card
    }

    /// 懒备好的：第一次调才读 `link_preview.json`、读环境变量里的代理；读不懂的记一条 `WARN`，不再试。
    fn ready(&self) -> Result<&Ready, NotReady> {
        self.ready
            .get_or_init(|| match rules::load(&self.resources) {
                Ok(rules) => {
                    let (guard, proxies) = self.network();
                    Ok(Ready {
                        fetcher: Fetcher::new(&rules, guard, proxies),
                        sites: rules.sites.clone(),
                        ttwid: sites::Ttwid::default(),
                        challenge_titles: rules.challenge.titles.clone(),
                        clip: rules.clip,
                        remember: Remember::new(Keep {
                            found: rules.found,
                            no_preview: rules.no_preview,
                            unreachable: rules.unreachable,
                            entries: rules.entries,
                        }),
                    })
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, error = %error, "not ready");
                    Err(())
                }
            })
            .as_ref()
            .map_err(|()| NotReady)
    }

    /// 闸和代理：平时是没有口子的闸、环境变量里的代理；测试照给的。
    fn network(&self) -> (Guard, Proxies) {
        #[cfg(any(test, feature = "testkit"))]
        if let Some(testing) = &self.testing {
            let proxies = Proxies::given(testing.proxy.as_deref(), &testing.no_proxy);
            return (Guard::testing(testing.clone()), proxies);
        }
        (Guard::new(), Proxies::from_env())
    }
}
