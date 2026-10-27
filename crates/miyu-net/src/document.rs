//! 抓一份网页给她读（施工 N-1，设计 36 第一节第 3 条，`net.md`「抓网页」）：`web_fetch` 那件工具的里子。
//!
//! 和链接卡片同一道闸、同一套代理、同一份规矩文件（`link_preview.json` 的 `document`），各自一个 [`Fetcher`]：卡片只读到
//! `</head>`，这里整份读（有上限）。抓到的照 `convert.rs` 换成字，同一个地址在 `keep` 之内再要（翻页）照记着的给，不重抓。
//! 换 Markdown 在阻塞线程里做，同时至多两份：一份大网页建树、换字要几百毫秒，不能占着异步的线程。

mod convert;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use reqwest::Url;
use tokio::sync::Semaphore;

use crate::fetch::{Fetcher, Hop, Missed};
use crate::rules::{self, Reading};
use crate::{Asker, TARGET};
use convert::{Kind, Unconverted};
use miyu_http::guard::Guard;
use miyu_http::proxy::Proxies;

/// 同时至多几份在换 Markdown。
static CONVERTING: Semaphore = Semaphore::const_new(2);

/// 读成的一份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// 最后落到的地址（跟过跳转的）。
    pub url: String,
    /// 网页的标题，没有的没有。
    pub title: Option<String>,
    /// 给她读的字：网页换成了 Markdown，文字照原样。
    pub text: String,
    /// 超了大小的上限、截了后面。
    pub cut: bool,
    /// 自己抓不下来、请哪一家代抓的（施工 N-4）：给人看的名字。自己抓的没有。
    pub via: Option<String>,
}

/// 读不成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Miss {
    /// 不是网址。
    NotAUrl,
    /// 不是 http、https。
    Scheme,
    /// 过不了闸：内网、本机、链路本地这些地址。
    Blocked,
    /// 超时、连不上。
    Unreachable,
    /// 跳转太多。
    Redirects,
    /// 人机验证、挡爬虫的页面。
    Challenge,
    /// 回了 2xx 以外的状态码。
    Status(u16),
    /// 不是读得了的种类：图、PDF、压缩包这些；带着媒体类型。
    Unsupported(String),
    /// 网页太大、嵌套太深，换不成字。
    TooComplex,
    /// 换完一个字都没有。
    Empty,
    /// 规矩文件读不懂（原话记运行日志）。
    NotReady,
}

/// 抓网页的家底：资源目录在哪、懒备好的抓取、记着的几份。一个核心一份。
pub struct Pages {
    resources: PathBuf,
    ready: OnceLock<Result<Ready, ()>>,
    #[cfg(any(test, feature = "testkit"))]
    testing: Option<crate::testkit::Testing>,
}

/// 读了规矩以后备好的。
struct Ready {
    fetcher: Fetcher,
    reading: Reading,
    kept: Mutex<HashMap<String, (Instant, Arc<Read>)>>,
}

impl Pages {
    /// 一份家底：只记资源目录在哪，不碰磁盘。规矩等第一次 [`Pages::read`] 再读。
    pub fn new(resources: &Path) -> Pages {
        Pages {
            resources: resources.to_path_buf(),
            ready: OnceLock::new(),
            #[cfg(any(test, feature = "testkit"))]
            testing: None,
        }
    }

    /// 测试用：闸和代理照 `testing`。生产的构建里没有这个方法。
    #[cfg(any(test, feature = "testkit"))]
    #[must_use]
    pub fn testing(mut self, testing: crate::testkit::Testing) -> Pages {
        self.testing = Some(testing);
        self
    }

    /// 一页至多几个字：规矩读不懂的照 30000。
    pub fn page_chars(&self) -> usize {
        self.ready()
            .map_or(30_000, |ready| ready.reading.page_chars)
    }

    /// 读一个网址：去掉前后空白读成网址、过闸、抓、换成字。同一个地址在 `keep` 之内照记着的给。
    ///
    /// # Errors
    ///
    /// 见 [`Miss`]。
    pub async fn read(&self, text: &str, asker: &Asker) -> Result<Arc<Read>, Miss> {
        let Ok(url) = Url::parse(text.trim()) else {
            return Err(Miss::NotAUrl);
        };
        if !matches!(url.scheme(), "http" | "https") {
            return Err(Miss::Scheme);
        }
        let ready = self.ready().ok_or(Miss::NotReady)?;
        // 内部域名先查，再看记着的：机器主人抓到的不给别人（施工 MB-2）。
        if asker.internal(url.host_str().unwrap_or_default()) {
            return Err(Miss::Blocked);
        }
        let key = key(asker, &url);
        if let Some(kept) = ready.kept(&key) {
            return Ok(kept);
        }
        let read = Arc::new(fetch(ready, &url, asker).await?);
        ready.keep(key, Arc::clone(&read));
        Ok(read)
    }

    /// 记下别处读成的一份（施工 N-4：代抓到的），翻页不再去抓。网址读不成、规矩读不懂的不记。
    pub fn keep(&self, text: &str, asker: &Asker, read: Arc<Read>) {
        if let (Ok(url), Some(ready)) = (Url::parse(text.trim()), self.ready()) {
            ready.keep(key(asker, &url), read);
        }
    }

    fn ready(&self) -> Option<&Ready> {
        self.ready
            .get_or_init(|| match rules::load(&self.resources) {
                Ok(rules) => {
                    let (guard, proxies) = self.network();
                    Ok(Ready {
                        fetcher: Fetcher::new(&rules, guard, proxies),
                        reading: rules.reading,
                        kept: Mutex::new(HashMap::new()),
                    })
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, error = %error, "web fetch not ready");
                    Err(())
                }
            })
            .as_ref()
            .ok()
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

/// 记着的那一份照什么找：同一个地址，替别人抓的另记一份（施工 MB-2：机器主人经代理抓到的不给别人）。
fn key(asker: &Asker, url: &Url) -> String {
    match asker {
        Asker::Owner => url.as_str().to_string(),
        Asker::Other { .. } => format!("other {url}"),
    }
}

impl Ready {
    /// 记着的、没过期的那一份。
    fn kept(&self, key: &str) -> Option<Arc<Read>> {
        let kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        kept.get(key)
            .filter(|(at, _)| at.elapsed() < self.reading.keep)
            .map(|(_, read)| Arc::clone(read))
    }

    /// 记下一份；满了先整个清空。
    fn keep(&self, key: String, read: Arc<Read>) {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if kept.len() >= self.reading.keep_entries {
            kept.clear();
        }
        kept.insert(key, (Instant::now(), read));
    }
}

/// 抓、认、解、换。
async fn fetch(ready: &Ready, url: &Url, asker: &Asker) -> Result<Read, Miss> {
    let fetched = ready
        .fetcher
        .document(url, asker)
        .await
        .map_err(|missed| match missed {
            Missed::Hop(Hop::Blocked) => Miss::Blocked,
            Missed::Hop(Hop::Unreachable) => Miss::Unreachable,
            Missed::Hop(Hop::Redirects) => Miss::Redirects,
            Missed::Challenge => Miss::Challenge,
            Missed::Status(status) => Miss::Status(status),
        })?;
    let kind = convert::kind(&fetched.media, &fetched.bytes)
        .ok_or_else(|| Miss::Unsupported(fetched.media.clone()))?;
    let decoded = convert::decode(&fetched.bytes, fetched.charset.as_deref(), kind);
    let (title, text) = match kind {
        Kind::Text => (None, convert::tidy(&decoded)),
        Kind::Html => {
            let _turn = CONVERTING.acquire().await.map_err(|_| Miss::TooComplex)?;
            let title = convert::title(&decoded);
            let text = tokio::task::spawn_blocking(move || convert::markdown(&decoded))
                .await
                .map_err(|_| Miss::TooComplex)?
                .map_err(|unconverted| match unconverted {
                    Unconverted::TooComplex | Unconverted::Unparsed => Miss::TooComplex,
                })?;
            (title, text)
        }
    };
    if text.is_empty() {
        return Err(Miss::Empty);
    }
    Ok(Read {
        url: fetched.url.to_string(),
        title,
        text,
        cut: fetched.cut,
        via: None,
    })
}
