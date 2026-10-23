//! 站要的另外几样请求（施工 N-6）：`POST` 一次、交回回应头里某个 cookie 的值。抖音拿匿名的 `ttwid` 用：它的 `Domain` 是
//! `bytedance.com`，不会自己带给别的站，调的一方手拼 `Cookie` 头。同一道闸、同一套代理、复用的客户端；不跟跳转。

use reqwest::Url;
use reqwest::header::{ACCEPT, CONTENT_TYPE, SET_COOKIE, USER_AGENT};

use super::Fetcher;
use crate::Asker;

impl Fetcher {
    /// 照 `user_agent` 把 `body`（JSON 原文）`POST` 给 `url`，交回回应头 `Set-Cookie` 里 `name=` 的值（原样，到 `;` 为止）。
    /// 照 `api` 的时限。过不了闸、连不上、回的不是 2xx、没有这个 cookie 的、值是空的：`None`。
    pub(crate) async fn cookie_from(
        &self,
        url: &Url,
        user_agent: &str,
        body: &str,
        name: &str,
    ) -> Option<String> {
        let (client, left) = self.client_for(url, &self.api, &Asker::Owner).await.ok()?;
        let response = client
            .post(url.clone())
            .timeout(left)
            .header(USER_AGENT, user_agent)
            .header(ACCEPT, &self.api.accept)
            .header(CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let prefix = format!("{name}=");
        response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .find_map(|cookie| {
                let value = cookie.trim().strip_prefix(&prefix)?;
                let value = value.split(';').next().unwrap_or_default().trim();
                (!value.is_empty()).then(|| value.to_string())
            })
    }
}
