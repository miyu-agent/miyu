//! 拿发行版订阅和文件：经 `miyu-http` 的一次 GET，照环境变量走代理。测试换成替身（[`Net`]）。都是 `github.com` 上的普通网页和
//! 下载，不走 GitHub API，没有匿名每小时 60 次的额度（施工 UP-4）。

use std::time::Duration;

/// 没拿到的原因，给人看时分开说。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NetError {
    /// 连不上、超时、读到一半断了：系统的原话。
    Offline(String),
    /// 回的不是 2xx：状态码（`github.com` 自己偶尔也回 429，照状态码说）。
    Status(u16),
}

/// 拿一个地址的全部字节。
pub(crate) trait Net {
    /// GET `url`，最多 `limit` 字节。`quick` 的（订阅）最多等 30 秒，别的（文件）15 分钟。
    fn get(&self, url: &str, limit: usize, quick: bool) -> Result<Vec<u8>, NetError>;
}

/// 真的那一个：一个单线程的运行时，一个 GET 用的客户端。
pub(crate) struct Real {
    runtime: tokio::runtime::Runtime,
    client: miyu_http::Client,
}

impl Real {
    /// 造一个；造不出来的交回原话。
    pub(crate) fn new() -> Result<Real, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let client = miyu_http::fetcher(miyu_http::Proxy::FromEnvironment)
            .map_err(|error| error.to_string())?;
        Ok(Real { runtime, client })
    }
}

/// 订阅最多多久；文件（压缩包几十 MB）最多多久。
const QUICK_TIMEOUT: Duration = Duration::from_secs(30);
const FILE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

impl Net for Real {
    fn get(&self, url: &str, limit: usize, quick: bool) -> Result<Vec<u8>, NetError> {
        let got = self.runtime.block_on(miyu_http::get_full(miyu_http::Get {
            client: &self.client,
            url,
            headers: &[],
            etag: None,
            timeout: if quick { QUICK_TIMEOUT } else { FILE_TIMEOUT },
            limit,
        }));
        match got {
            Ok(miyu_http::Got::Body { bytes, .. }) => Ok(bytes),
            Ok(miyu_http::Got::NotModified) => Err(NetError::Status(304)),
            Err(failed) => Err(classify(&failed)),
        }
    }
}

/// 没拿到的分开：没有状态码的是连不上，别的照状态码。
pub(crate) fn classify(failed: &miyu_http::Failed) -> NetError {
    match failed.status {
        None => NetError::Offline(failed.message.clone()),
        Some(status) => NetError::Status(status),
    }
}

#[cfg(test)]
mod tests;
