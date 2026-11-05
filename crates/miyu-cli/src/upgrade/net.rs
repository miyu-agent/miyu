//! 拿发布列表和文件：经 `miyu-http` 的一次 GET，照环境变量走代理。测试换成替身（[`Net`]）。

use std::time::Duration;

/// 没拿到的原因，给人看时分开说。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NetError {
    /// 连不上、超时、读到一半断了：系统的原话。
    Offline(String),
    /// GitHub 限流（匿名每小时 60 次）：什么时候恢复（Unix 秒），没说的没有。
    Limited(Option<i64>),
    /// 回的不是 2xx：状态码。
    Status(u16),
}

/// 拿一个地址的全部字节。
pub(crate) trait Net {
    /// GET `url`，最多 `limit` 字节。`api` 的是 GitHub API（带它要的头）。
    fn get(&self, url: &str, limit: usize, api: bool) -> Result<Vec<u8>, NetError>;
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

/// API 的请求最多多久；文件（压缩包几十 MB）最多多久。
const API_TIMEOUT: Duration = Duration::from_secs(30);
const FILE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

impl Net for Real {
    fn get(&self, url: &str, limit: usize, api: bool) -> Result<Vec<u8>, NetError> {
        let headers = if api {
            vec![
                (
                    "Accept".to_string(),
                    "application/vnd.github+json".to_string(),
                ),
                ("X-GitHub-Api-Version".to_string(), "2022-11-28".to_string()),
            ]
        } else {
            Vec::new()
        };
        let got = self.runtime.block_on(miyu_http::get_full(miyu_http::Get {
            client: &self.client,
            url,
            headers: &headers,
            etag: None,
            timeout: if api { API_TIMEOUT } else { FILE_TIMEOUT },
            limit,
        }));
        match got {
            Ok(miyu_http::Got::Body { bytes, .. }) => Ok(bytes),
            Ok(miyu_http::Got::NotModified) => Err(NetError::Status(304)),
            Err(failed) => Err(classify(&failed)),
        }
    }
}

/// 没拿到的分开：没有状态码的是连不上；403、429 带着 `x-ratelimit-remaining: 0` 的是限流；别的照状态码。
pub(crate) fn classify(failed: &miyu_http::Failed) -> NetError {
    let Some(status) = failed.status else {
        return NetError::Offline(failed.message.clone());
    };
    let header = |name: &str| {
        failed
            .headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    if matches!(status, 403 | 429) && header("x-ratelimit-remaining") == Some("0") {
        return NetError::Limited(header("x-ratelimit-reset").and_then(|reset| reset.parse().ok()));
    }
    NetError::Status(status)
}

#[cfg(test)]
mod tests;
