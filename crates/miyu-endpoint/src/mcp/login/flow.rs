//! `mcp.login {service}`（施工 X-42 上）：找服务（个人设置里的先，再找包里带的）、照挑战找授权服务器、拿客户端（存着的、同一个
//! 授权服务器的照用，没有的动态注册）、绑回调的端口（存着的那一个，绑不上的换一个随机的、动态注册来的照新端口重新注册一次）、拼
//! 授权页交回；后台等回调（最多 10 分钟）：`state`、`iss` 对上了才换令牌，换到的照 `secret.set` 那一路存进
//! `mcp-oauth-<服务名>`，密钥变了的那一路重起服务。同一个服务又登录的，上一次在等的那个停掉。浏览器不在核心那台机器上的，贴回来
//! 的地址（`mcp.login_finish`，施工 X-42 下）交给同一个等着的任务，和本机回调同一处判。要的范围是存着的加上这一次要的（施工
//! X-42 下，规范的 step-up）。
//!
//! 授权服务器回的错误说明是不可信的字：截过（`miyu_mcp::oauth`），只进 `data.why`，不进日志、不进给模型的说明。

use std::sync::{Arc, PoisonError, Weak};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::{McpServer, Reach};
use miyu_kernel::id::CommandId;
use miyu_mcp::oauth::{
    Asked, Callback, Discovered, Exchange, Listener, Pkce, Registered, Reply, Trouble,
    authorize_url, discover, exchange, iss_ok, listen, random_state, register,
};
use miyu_store::scripts::HOME_PACKAGE;
use tokio::sync::{mpsc, oneshot};

use super::{Ongoing, Pasted, Stored, now, secret_name, stored};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 运行日志的目标。
const TARGET: &str = "miyu::mcp";

/// 等回调最多多久。
const WAIT: Duration = Duration::from_secs(600);

/// `mcp.login` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoginParams {
    /// 服务名。
    service: String,
}

/// `mcp.login`：交回 `{"url"}`（授权页），人在浏览器里点同意；后台等回调、换令牌、存。
///
/// # Errors
///
/// 没有这个服务的 `unknown_server`；不是网址的、表里自己写了令牌的 `bad_params`；找不到、用不了授权服务器的、拒了的
/// `oauth_failed`（`data.why`）；没有客户端、授权服务器又不认动态注册的 `oauth_unregistered`。
pub(crate) async fn login(
    core: &Arc<Core>,
    peer: Peer,
    cause: &CommandId,
    params: LoginParams,
) -> Result<Value, Refusal> {
    let (package, server) = find(core, &params.service).ok_or(Refusal::UNKNOWN_SERVER)?;
    let Reach::Url { url, .. } = &server.reach else {
        return Err(Refusal::BAD_PARAMS);
    };
    if super::explicit(&server) {
        return Err(Refusal::BAD_PARAMS);
    }
    let http = client(url)?;
    let challenge = core
        .mcp
        .login_challenge(&package, &server.name)
        .unwrap_or_default();
    let found = discover(&http, url, &challenge).await.map_err(refused)?;
    let kept = stored(&core.config(), &server.name)
        .filter(|kept| kept.issuer == found.authorization.issuer);
    let scope = merged(
        kept.as_ref().and_then(|kept| kept.scope.as_deref()),
        found.scope.as_deref(),
    );
    let (listener, client, registered) = prepared(&http, &found, kept.as_ref()).await?;
    let pkce = Pkce::new().map_err(|_| Refusal::INTERNAL)?;
    let state = random_state().map_err(|_| Refusal::INTERNAL)?;
    let (sender, receiver) = mpsc::unbounded_channel();
    let redirect_uri = listener.redirect_uri();
    let page = authorize_url(
        &found.authorization,
        &Asked {
            client_id: &client.client_id,
            redirect_uri: &redirect_uri,
            scope: scope.as_deref(),
            resource: &found.resource,
            state: &state,
            challenge: &pkce.challenge,
        },
    );
    tracing::info!(target: TARGET, service = server.name.as_str(), port = listener.port(), "mcp oauth login started");
    let waiting = Waiting {
        core: Arc::downgrade(core),
        peer,
        cause: cause.clone(),
        service: server.name.clone(),
        http,
        found,
        client,
        registered,
        pkce,
        state,
        redirect_uri,
        pasted: receiver,
    };
    let task = tokio::spawn(waiting.wait(listener));
    if let Some(earlier) = core
        .mcp
        .logins
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            server.name.clone(),
            Ongoing {
                task,
                pasted: sender,
            },
        )
    {
        earlier.task.abort();
    }
    Ok(json!({ "url": page }))
}

/// 叫 `name` 的服务：个人设置里的先（同名的盖住包里的），再找装着的包带的。交回包的编号和那一张表。
fn find(core: &Core, name: &str) -> Option<(String, McpServer)> {
    if let Some(server) = core
        .home_mcp
        .now()
        .iter()
        .find(|server| server.name == name)
    {
        return Some((HOME_PACKAGE.to_string(), server.clone()));
    }
    let packages = core.system_packages();
    crate::packages::manifests(&packages).find_map(|(id, manifest)| {
        manifest
            .mcp
            .iter()
            .find(|server| server.name == name)
            .map(|server| (id.to_string(), server.clone()))
    })
}

/// 连授权服务器用的客户端：本机的地址不走代理，同连服务那一条。
fn client(url: &str) -> Result<miyu_http::Client, Refusal> {
    let proxy = match miyu_http::is_loopback_url(url) {
        true => miyu_http::Proxy::Off,
        false => miyu_http::Proxy::FromEnvironment,
    };
    miyu_http::client(proxy).map_err(|_| Refusal::INTERNAL)
}

/// 回调听着、客户端拿到：存着的照用、端口绑同一个；绑不上的换随机端口，动态注册来的照新端口重新注册一次；没有客户端的动态注册。
/// 交回听着的、客户端、是不是动态注册来的。
async fn prepared(
    http: &miyu_http::Client,
    found: &Discovered,
    kept: Option<&Stored>,
) -> Result<(Listener, Registered, bool), Refusal> {
    if let Some(kept) = kept {
        let client = Registered {
            client_id: kept.client_id.clone(),
            client_secret: kept.client_secret.clone(),
        };
        if let Ok(listener) = listen(Some(kept.port)).await {
            return Ok((listener, client, kept.registered));
        }
        let listener = listen(None).await.map_err(|_| Refusal::INTERNAL)?;
        if !kept.registered {
            return Ok((listener, client, false));
        }
        let client = register(http, &found.authorization, &listener.redirect_uri())
            .await
            .map_err(refused)?;
        return Ok((listener, client, true));
    }
    let listener = listen(None).await.map_err(|_| Refusal::INTERNAL)?;
    let client = register(http, &found.authorization, &listener.redirect_uri())
        .await
        .map_err(refused)?;
    Ok((listener, client, true))
}

/// 登录这几步走不下去：没有客户端的 `oauth_unregistered`，别的 `oauth_failed` 带一句为什么（授权服务器的原话截过）。
fn refused(trouble: Trouble) -> Refusal {
    let why = match trouble {
        Trouble::Unregistered => return Refusal::OAUTH_UNREGISTERED,
        Trouble::Unreachable(why) => why,
        Trouble::NoMetadata => "no authorization metadata found".to_string(),
        Trouble::Metadata(problem) => format!("authorization metadata unusable: {problem:?}"),
        Trouble::NoAuthorizationServer => "no authorization server listed".to_string(),
        Trouble::Refused { error, description } => match description {
            Some(description) => format!("{error}: {description}"),
            None => error,
        },
    };
    Refusal::OAUTH_FAILED.because(why)
}

/// 在等回调的这一次登录。
struct Waiting {
    core: Weak<Core>,
    peer: Peer,
    cause: CommandId,
    service: String,
    http: miyu_http::Client,
    found: Discovered,
    client: Registered,
    registered: bool,
    pkce: Pkce,
    state: String,
    redirect_uri: String,
    /// 贴回来的地址（施工 X-42 下，`mcp.login_finish`）。
    pasted: mpsc::UnboundedReceiver<Pasted>,
}

/// 回调是从哪来的：浏览器到了本机（回它一页），还是人贴回来的（回 `mcp.login_finish`）。
enum Answer {
    Browser(Reply),
    Pasted(oneshot::Sender<Result<(), Refusal>>),
}

impl Answer {
    /// 交回结局：浏览器的回 `page`，贴回来的回成了没成。
    async fn send(self, ok: bool, page: &str) {
        match self {
            Answer::Browser(reply) => reply.send(ok, page).await,
            Answer::Pasted(answer) => {
                let told = if ok {
                    Ok(())
                } else {
                    Err(Refusal::OAUTH_FAILED)
                };
                if answer.send(told).is_err() {
                    // 贴的那一头已经不等了。
                }
            }
        }
    }
}

/// 下次登录要的范围（施工 X-42 下，规范的 step-up）：存着的加上这一次要的，去重、照先后；都没有的没有。
fn merged(kept: Option<&str>, asked: Option<&str>) -> Option<String> {
    let mut all: Vec<&str> = Vec::new();
    for scope in kept
        .into_iter()
        .chain(asked)
        .flat_map(str::split_whitespace)
    {
        if !all.contains(&scope) {
            all.push(scope);
        }
    }
    (!all.is_empty()).then(|| all.join(" "))
}

impl Waiting {
    /// 等回调：`state` 对不上的不认、接着等；对上了的照 `iss` 判，换令牌、存。到时没来的记一行就停。
    async fn wait(mut self, listener: Listener) {
        let service = self.service.clone();
        let service = service.as_str();
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            let (callback, reply) = tokio::select! {
                next = listener.next() => match next {
                    Ok((callback, reply)) => (callback, Answer::Browser(reply)),
                    Err(_) => {
                        tracing::warn!(target: TARGET, service, "mcp oauth callback failed");
                        return;
                    }
                },
                Some(pasted) = self.pasted.recv() => match miyu_mcp::oauth::pasted(&pasted.url) {
                    Some(callback) => (callback, Answer::Pasted(pasted.answer)),
                    None => {
                        Answer::Pasted(pasted.answer).send(false, "").await;
                        continue;
                    }
                },
                () = tokio::time::sleep_until(deadline) => {
                    tracing::info!(target: TARGET, service, "mcp oauth login timed out");
                    return;
                }
            };
            let (state, iss) = match &callback {
                Callback::Code { state, iss, .. } => (Some(state.as_str()), iss.as_deref()),
                Callback::Error { state, iss, .. } => (state.as_deref(), iss.as_deref()),
            };
            if state != Some(self.state.as_str()) {
                reply.send(false, &self.page("mcp/login-failed")).await;
                continue;
            }
            let authorization = &self.found.authorization;
            if !iss_ok(authorization.iss_supported, iss, &authorization.issuer) {
                tracing::warn!(target: TARGET, service, "mcp oauth issuer mismatch");
                reply.send(false, &self.page("mcp/login-failed")).await;
                return;
            }
            let Callback::Code { code, .. } = callback else {
                tracing::warn!(target: TARGET, service, "mcp oauth refused");
                reply.send(false, &self.page("mcp/login-failed")).await;
                return;
            };
            let stored = self.exchanged(&code, listener.port()).await;
            let ok = stored.is_some_and(|stored| self.store(&stored));
            let key = if ok {
                "mcp/login-done"
            } else {
                "mcp/login-failed"
            };
            reply.send(ok, &self.page(key)).await;
            return;
        }
    }

    /// 授权码换令牌，换到的连同客户端、端口写成存进密钥的那一份。换不到的记一行（只写 OAuth 的 `error`）。
    async fn exchanged(&self, code: &str, port: u16) -> Option<Stored> {
        let asked = Exchange {
            code,
            verifier: &self.pkce.verifier,
            redirect_uri: &self.redirect_uri,
            resource: &self.found.resource,
        };
        match exchange(&self.http, &self.found.authorization, &self.client, &asked).await {
            Ok(tokens) => {
                let now = now();
                Some(Stored {
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                    expires_at: tokens.expires_in.map(|seconds| now + seconds),
                    scope: tokens.scope,
                    issuer: self.found.authorization.issuer.clone(),
                    client_id: self.client.client_id.clone(),
                    client_secret: self.client.client_secret.clone(),
                    port,
                    registered: self.registered,
                    resource: Some(self.found.resource.clone()),
                })
            }
            Err(trouble) => {
                let error = match &trouble {
                    Trouble::Refused { error, .. } => error.as_str(),
                    _ => "unreachable",
                };
                tracing::warn!(target: TARGET, service = self.service.as_str(), error, "mcp oauth token not given");
                None
            }
        }
    }

    /// 存进密钥文件：照 `secret.set` 那一路（整份重写、0600、照版本防并发改），记在登录的这个人名下。存不成的记一行。
    fn store(&self, stored: &Stored) -> bool {
        let Some(core) = self.core.upgrade() else {
            return false;
        };
        let Ok(value) = serde_json::to_string(stored) else {
            return false;
        };
        let name = secret_name(&self.service);
        match crate::secrets::store(&core, self.peer, &self.cause, &name, &value) {
            Ok(()) => {
                tracing::info!(target: TARGET, service = self.service.as_str(), "mcp oauth logged in");
                true
            }
            Err(_) => {
                tracing::warn!(target: TARGET, service = self.service.as_str(), "mcp oauth token not stored");
                false
            }
        }
    }

    /// 回浏览器的那一页：照登录的人的语言。
    fn page(&self, key: &str) -> String {
        let text = self
            .core
            .upgrade()
            .and_then(|core| crate::config::methods::words(&core, self.peer.language).ok())
            .and_then(|words| miyu_config::Words::sentence(&words, key, &[]))
            .unwrap_or_default();
        format!("<!doctype html><meta charset=\"utf-8\"><title>Miyu</title><p>{text}</p>\n")
    }
}

impl super::super::Servers {
    /// 包 `package` 的服务 `service` 在等登录时的挑战：没在等的没有。
    pub(crate) fn login_challenge(
        &self,
        package: &str,
        service: &str,
    ) -> Option<miyu_mcp::oauth::Challenge> {
        self.slots()
            .get(&(package.to_string(), service.to_string()))?
            .shared
            .login()
    }
}
