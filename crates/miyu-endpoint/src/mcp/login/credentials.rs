//! 登录来的服务现场给令牌（施工 X-42 下，`docs/blueprint/mcp.md`「怎么走」第八条）：转接每个请求问一次。离 `expires_at` 不到
//! [`EARLY`] 秒的先续再给；回 `401` 的续一次（几个请求同时撞上的只续一次：有的授权服务器续一次就作废旧的 `refresh_token`）。
//! 续：照存着的 `issuer` 再取一次授权服务器的元数据，`grant_type=refresh_token`，带登录时的 `resource`、`scope`。成了的照密钥
//! 现有的写法存回 `mcp-oauth-<服务名>`（新给了 `refresh_token` 的换上，没给的留旧的），记下摘要：跟着配置的那一圈不当人改的、
//! 不重起服务。续不成的记一行 `WARN mcp oauth refresh failed`（只写 OAuth 的 `error` 码），转接照 `401` 停在 `needs_login`。

use std::sync::{Arc, Weak};

use tokio::sync::Mutex;

use miyu_mcp::http::{Credentials, Pending};
use miyu_mcp::oauth::{Refresh, Registered, Trouble, authorization_of, canonical, refresh};

use super::{Stored, now, secret_name, stored};
use crate::Core;

/// 运行日志的目标。
const TARGET: &str = "miyu::mcp";

/// 离到期不到这么多秒的先续。
const EARLY: u64 = 60;

/// 一个登录来的服务的令牌：存着的那一份，续期一次只跑一个。
pub(crate) struct Login {
    core: Weak<Core>,
    service: String,
    /// 服务的网址：存着的那一份没写 `resource` 的照它的规范网址。
    url: String,
    http: miyu_http::Client,
    stored: Mutex<Stored>,
}

impl Login {
    /// 服务 `service`（网址 `url`）存着的那一份：没登录过的没有。`http` 是连它的那个客户端，续期也用它。
    pub(crate) fn of(
        core: &Weak<Core>,
        service: &str,
        url: &str,
        http: miyu_http::Client,
    ) -> Option<Arc<dyn Credentials>> {
        let owner = core.upgrade()?;
        let kept = stored(&owner.config(), service)?;
        Some(Arc::new(Login {
            core: core.clone(),
            service: service.to_string(),
            url: url.to_string(),
            http,
            stored: Mutex::new(kept),
        }))
    }

    /// 续一次：成了的存回去，交回新的那一份；续不成的记一行，交回没有。
    async fn refreshed(&self, kept: &Stored) -> Option<Stored> {
        let service = self.service.as_str();
        let Some(refresh_token) = kept.refresh_token.as_deref() else {
            tracing::warn!(target: TARGET, service, error = "no_refresh_token", "mcp oauth refresh failed");
            return None;
        };
        let resource = kept
            .resource
            .clone()
            .or_else(|| canonical(&self.url))
            .unwrap_or_default();
        let client = Registered {
            client_id: kept.client_id.clone(),
            client_secret: kept.client_secret.clone(),
        };
        let asked = Refresh {
            refresh_token,
            resource: &resource,
            scope: kept.scope.as_deref(),
        };
        let renewed = match authorization_of(&self.http, &kept.issuer).await {
            Ok(authorization) => refresh(&self.http, &authorization, &client, &asked).await,
            Err(trouble) => Err(trouble),
        };
        match renewed {
            Ok(tokens) => {
                let next = Stored {
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token.or_else(|| kept.refresh_token.clone()),
                    expires_at: tokens.expires_in.map(|seconds| now() + seconds),
                    scope: tokens.scope.or_else(|| kept.scope.clone()),
                    resource: Some(resource),
                    ..kept.clone()
                };
                self.keep(&next);
                tracing::info!(target: TARGET, service, "mcp oauth refreshed");
                Some(next)
            }
            Err(trouble) => {
                let error = match &trouble {
                    Trouble::Refused { error, .. } => error.as_str(),
                    _ => "unreachable",
                };
                tracing::warn!(target: TARGET, service, error, "mcp oauth refresh failed");
                None
            }
        }
    }

    /// 存回密钥（整份重写、0600、照版本防并发改，撞了重读再写），先记下这一份的摘要。存不成的记一行，这一次照新令牌用。
    fn keep(&self, stored: &Stored) {
        let Some(core) = self.core.upgrade() else {
            return;
        };
        let Ok(value) = serde_json::to_string(stored) else {
            return;
        };
        let name = secret_name(&self.service);
        core.mcp.wrote(&name, &value);
        if crate::secrets::keep(&core, &name, &value).is_err() {
            tracing::warn!(target: TARGET, service = self.service.as_str(), "mcp oauth token not stored");
        }
    }
}

/// 存着的这一份是不是快过期了：没说什么时候过期的不算。
fn expiring(stored: &Stored) -> bool {
    stored
        .expires_at
        .is_some_and(|at| at <= now().saturating_add(EARLY))
}

impl Credentials for Login {
    fn token(&self) -> Pending<'_> {
        Box::pin(async move {
            let mut kept = self.stored.lock().await;
            if expiring(&kept)
                && let Some(next) = self.refreshed(&kept).await
            {
                *kept = next;
            }
            Some(kept.access_token.clone())
        })
    }

    fn renew<'a>(&'a self, rejected: &'a str) -> Pending<'a> {
        Box::pin(async move {
            let mut kept = self.stored.lock().await;
            if kept.access_token != rejected {
                return Some(kept.access_token.clone());
            }
            let next = self.refreshed(&kept).await?;
            *kept = next;
            Some(kept.access_token.clone())
        })
    }
}
