//! 替成员下东西（施工 U23 下下，设计 06 第八再又补第 2 条「她替成员上网不许连本机、局域网」）：成员自己加的软件源的索引、包
//! 文件照它拿。和 `miyu-net` 抓网页是同一套规矩（[`crate::guard`]、[`crate::proxy`]，`net.md`「怎么走」第 3 到 5 条）：
//!
//! - 每一跳先过闸，再看走不走代理。不走代理的钉住闸解析好的地址：查过的就是连上的，中间没有第二次解析（防 DNS rebinding）；
//!   本机解析不出来的连不上。走代理的交给代理（代理那头自己解析），本机解析出来有不是公网的照样拒。
//! - 跳转不交给 reqwest：自己跟，最多 [`REDIRECTS`] 跳，每一跳重新过闸。
//! - 只认 http、https，不带用户名、密码，不去 `localhost`、`*.local` 这些。
//!
//! 出错的原话是英文的一句，不带地址里的路径、参数。

use std::time::Duration;

use reqwest::header::LOCATION;
use reqwest::redirect::Policy;
use reqwest::{Client, Response, Url};

use crate::guard::{Guard, Resolved};
use crate::proxy::Proxies;

/// 最多跟几跳跳转。
pub const REDIRECTS: usize = 10;

/// 解析一个主机名最多等多久。
const RESOLVE: Duration = Duration::from_secs(10);

/// 连上一个地址最多等多久（同 [`crate::fetcher`]）。
const CONNECT: Duration = Duration::from_secs(10);

/// 只连公网的下载：闸、代理。
pub struct Guarded {
    guard: Guard,
    proxies: Proxies,
}

impl Guarded {
    /// 平时的：闸没有口子，代理照这时的环境变量。
    pub fn from_env() -> Guarded {
        Guarded {
            guard: Guard::new(),
            proxies: Proxies::from_env(),
        }
    }

    /// 测试的：闸、代理照 `testing`（回环当不当公网、照哪张表解析、代理照给的值）。
    #[cfg(any(test, feature = "testkit"))]
    pub fn testing(testing: crate::guard::Testing) -> Guarded {
        let proxies = Proxies::given(testing.proxy.as_deref(), &testing.no_proxy);
        Guarded {
            guard: Guard::testing(testing),
            proxies,
        }
    }

    /// GET `url`，跟着跳转走到头，交回 2xx 的响应（响应体还没读）。每一跳从过闸到收完响应头最多等 `budget`。
    ///
    /// # Errors
    ///
    /// 网址不对、过不了闸、解析不出来又不走代理、连不上、超时、跳转太多、跳转没写去哪、回的不是 2xx：英文的一句。
    pub async fn open(&self, url: &str, budget: Duration) -> Result<Response, String> {
        let mut url = Url::parse(url).map_err(|_| "not a valid address".to_string())?;
        for _ in 0..=REDIRECTS {
            let client = self.client_for(&url).await?;
            let response = tokio::time::timeout(budget, client.get(url.clone()).send())
                .await
                .map_err(|_| "timed out".to_string())?
                .map_err(|error| error.without_url().to_string())?;
            let status = response.status();
            if status.is_redirection() {
                let next = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|location| url.join(location).ok())
                    .ok_or_else(|| "a redirect without a usable Location".to_string())?;
                url = next;
                continue;
            }
            if !status.is_success() {
                return Err(format!("HTTP {}", status.as_u16()));
            }
            return Ok(response);
        }
        Err("too many redirects".to_string())
    }

    /// GET `url` 整个读完：同 [`Guarded::open`]，响应体超过 `limit` 字节的不要，从过闸到读完最多 `budget`。
    ///
    /// # Errors
    ///
    /// 同 [`Guarded::open`]；超过上限、读到一半断了、超时。
    pub async fn get(&self, url: &str, limit: usize, budget: Duration) -> Result<Vec<u8>, String> {
        let read = async {
            let mut response = self.open(url, budget).await?;
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| error.without_url().to_string())?
            {
                if bytes.len() + chunk.len() > limit {
                    return Err(format!("larger than {limit} bytes"));
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(bytes)
        };
        tokio::time::timeout(budget, read)
            .await
            .map_err(|_| "timed out".to_string())?
    }

    /// 这一跳的客户端：先过闸，再照走不走代理造。跳转一个都不跟（[`Guarded::open`] 自己跟）。
    async fn client_for(&self, url: &Url) -> Result<Client, String> {
        let resolved = self
            .guard
            .pass(url, RESOLVE)
            .await
            .map_err(|_| "not a public address".to_string())?;
        let builder = Client::builder()
            .use_preconfigured_tls(crate::tls())
            .user_agent(concat!("miyu/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(CONNECT)
            .redirect(Policy::none());
        let builder = match (self.proxies.via(url), resolved) {
            // 设了一个代理，reqwest 就不再自己照环境变量找代理：这个客户端发的只经它。
            (Some(via), _) => builder.proxy(via.proxy().map_err(|error| error.to_string())?),
            (None, Resolved::Literal) => builder.no_proxy(),
            (None, Resolved::Addresses(addresses)) => {
                let host = url.host_str().unwrap_or_default();
                builder.no_proxy().resolve_to_addrs(host, &addresses)
            }
            (None, Resolved::Nowhere) => return Err("the host could not be resolved".to_string()),
        };
        builder.build().map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests;
