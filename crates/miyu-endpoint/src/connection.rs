//! 一个连接（`docs/designs/04-核心协议.md` 第三节「一次连接的全过程」）：一行一条消息，第一条必须是
//! `hello`，之后一条条照方法办。
//!
//! 读和写分开：读的一头一条条办请求；写的一头从一个有上限的队列里取出一行行写出去。订阅的推送由各自的
//! 转发任务放进同一个队列（施工 3-8 中）。头读得慢，队列满了，转发任务就不再从会话那里拿，会话那边的
//! 队列满了就掉队：核心和会话都不等这个头（第七节）。
//!
//! 分块上传跟着连接走（施工 W-5）：这个连接的上传表（[`crate::uploads::Uploads`]）住在读的一头的循环里，
//! 和请求一条条办；连接断了，循环结束前把它开着的上传全部作废、删暂存文件。
//!
//! 身份（施工 W-8，`web-module.md`「怎么走」第一条）：握手时定这个连接是怎么认出来的（[`Via`]）。用一次性码连上的只能调
//! `hello`、`human.get`、`account.setup`，别的回 `setup_first`；设好了换成登录令牌的连接。用登录令牌、密码连上的收作废的
//! 广播，作废了它靠的那个令牌就断开。
//!
//! 登记成在后台答的查询（`link.preview`，施工 W-7，`net.md`「怎么走」第 11 条）是一条条办的例外：交给这个连接自己的
//! 一组后台任务，接着读下一行；办完了回应照 `id` 对上，直接放进写队列。连接断了，这组任务一起停：不然连接走了
//! 还在抓，核心一直不算空闲。

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;

use miyu_kernel::id::SessionId;

mod accounts;
mod checks;
mod gate;
mod reach;
mod rights;
mod streams;

pub use reach::{Reach, serve_with};
#[cfg(test)]
mod tests;

use streams::{Stream, stream_of};

use crate::hello::{Caller, Shaken, hello};
use crate::login::{self, Revoked, Via};
use crate::methods;
use crate::queries::Handler;
use crate::refusal::{Locale, Refusal};
use crate::reverse::Peer;
use crate::subscriptions::{Later, Subscriptions, Target};
use crate::uploads::Uploads;
use crate::wire::{self, Incoming, Read, Request};
use crate::{Connected, Core};

/// 写队列能攒多少行：满了，转发任务就等着，会话那边掉队（`04-核心协议.md` 第七节）。
const QUEUE: usize = 256;

/// 照协议和这个连接说话，直到对方关了、握手没过，或者读写出错。字节流从哪来不管：本机套接字、命名
/// 管道、以后的 WebSocket、测试里的内存管道都行。
pub async fn serve<S>(stream: S, core: Arc<Core>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    serve_with(stream, core, Reach::Local).await;
}

/// 同 [`serve`]，对面是核心亲手拉起的扩展（施工 9-4 上）：握手不看凭据，握成了往 `shook` 说一声。
pub(crate) async fn serve_spawned<S>(
    stream: S,
    core: Arc<Core>,
    shook: oneshot::Sender<()>,
    package: String,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    serve_from(stream, core, Some((shook, package)), Reach::Local).await;
}

/// 两种连接共用的：`spawned` 有的是核心亲手拉起的扩展：握成了告诉看管的那一头，和它是哪个包（施工 9-4 下下）。
async fn serve_from<S>(
    stream: S,
    core: Arc<Core>,
    spawned: Option<(oneshot::Sender<()>, String)>,
    reach: Reach,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let _connected = Connected::new(Arc::clone(&core));
    let (read, write) = tokio::io::split(stream);
    let (out, lines) = mpsc::channel(QUEUE);
    tokio::join!(
        read_all(read, core, out, (spawned, reach)),
        write_all(write, lines)
    );
}

/// 写的一头：一行行写出去，写不出去就停。
async fn write_all<W: AsyncWrite + Unpin>(mut write: W, mut lines: mpsc::Receiver<String>) {
    while let Some(line) = lines.recv().await {
        let written = async {
            write.write_all(line.as_bytes()).await?;
            write.write_all(b"\n").await?;
            write.flush().await
        };
        if written.await.is_err() {
            return;
        }
    }
}

/// 读的一头：一条条办请求，回应放进写队列（订阅了的会话，经它的转发任务）。`spawned`、`reach` 见 [`serve_from`]。
async fn read_all<R: AsyncRead + Unpin>(
    read: R,
    core: Arc<Core>,
    out: mpsc::Sender<String>,
    (mut spawned, mut reach): (Option<(oneshot::Sender<()>, String)>, Reach),
) {
    let from_core = spawned.is_some();
    // 核心拉起的扩展是哪个包（施工 9-4 下下）：握手交它自己的配置，之后变了推。
    let package = spawned.as_ref().map(|(_, package)| package.clone());
    // 这个连接是谁（施工 O-4 下、Y-5 中上）：核心拉起的带进程的包是它自己的账号，别的是管理员；没声明系统账号的包替机主干活。
    // 核心往这个连接发的请求（施工 O-2 上，`reverse.rs`）：提供者的工具经它反向调用。
    let reverse = Peer::new(out.clone());
    let account = core.account_of(package.as_deref());
    let mut caller = Caller {
        reverse: reverse.clone(),
        serving: core.serving(&account),
        account,
        package: package.clone(),
    };
    // 它手里有哪些能力（施工 X-30；Y-5 下上起照组表）：批、升级都会重新拉起，一个连接算一次就够。
    let declared = package.as_deref().map_or_else(Vec::new, |package| {
        rights::declared(&core, package, caller.serving.as_ref())
    });
    let mut handing: Option<Handing> = None;
    let mut reader = BufReader::new(read);
    let mut shaken: Option<Shaken> = None;
    // 这个连接是怎么认出来的（施工 W-8）：用登录令牌、密码连上的另收作废的广播。
    let mut via: Option<Via> = None;
    // 设置那一段（施工 Y-12 中上）谁都收：注册了机主就一起断开。
    let mut revoked: Option<tokio::sync::broadcast::Receiver<Revoked>> =
        core.setting_up().then(|| core.identity.revoked());
    let mut subscriptions = Subscriptions::default();
    // 这个连接上的分块上传（施工 W-5）：和请求一条条办，不用锁。
    let mut uploads = Uploads::new(core.upload_idle);
    // 这个连接在后台答的请求（施工 W-7）：连接断了，丢掉它就一起停了。
    let mut background = JoinSet::new();
    // 握手的期限（施工 4-9 再补三上）：连上以后这么久还没握手成的，断开。
    let deadline = tokio::time::Instant::now() + core.hello_wait;
    loop {
        let read = match (shaken, revoked.as_mut()) {
            (Some(_), Some(revoked)) => {
                let mine = via
                    .as_ref()
                    .and_then(Via::login)
                    .unwrap_or_default()
                    .to_string();
                tokio::select! {
                    read = wire::read_line(&mut reader) => read,
                    () = until_revoked(revoked, &mine, &caller.account) => {
                        tracing::info!(target: "miyu::endpoint", "login revoked, closed");
                        break;
                    }
                }
            }
            (Some(_), None) => wire::read_line(&mut reader).await,
            (None, _) => {
                match tokio::time::timeout_at(deadline, wire::read_line(&mut reader)).await {
                    Ok(read) => read,
                    Err(_) => {
                        tracing::info!(target: "miyu::endpoint", "no hello, closed");
                        break;
                    }
                }
            }
        };
        let line = match read {
            Ok(Read::Line(line)) => line,
            Ok(Read::TooLong) => {
                let locale = shaken.map_or(Locale::En, |shaken| shaken.now(&core).locale);
                // 超长的读不完，行界也找不回来了：回一句读不懂，断开。
                tracing::warn!(target: "miyu::endpoint", "line too long, closed");
                send(&out, wire::error(Value::Null, Refusal::PARSE, locale)).await;
                break;
            }
            Ok(Read::Closed) | Err(_) => break,
        };
        // 每说一句都照这时的 `ui.language` 重算这个连接的语言（施工 8-4）。
        let peer = shaken.map(|shaken| shaken.now(&core));
        let locale = peer.map_or(Locale::En, |peer| peer.locale);
        let request = match wire::parse(&line) {
            Incoming::Request(request) => request,
            Incoming::Notification(notice) => {
                reach.heard(&notice, shaken.is_some());
                continue;
            }
            Incoming::Response(response) => {
                reverse.answer(response);
                continue;
            }
            Incoming::Bad(id, refusal) => {
                if !send(&out, wire::error(id, refusal, locale)).await {
                    break;
                }
                continue;
            }
        };
        tracing::debug!(target: "miyu::endpoint", method = request.method.as_str(), "request");
        core.activity.touch();
        reap(&mut background);
        // 办之前过的几道：凭据、本机才能用的、扩展的能力、会话只给属主（施工 Y-2 上，`checks.rs`）。
        let who = checks::Who {
            via: via.as_ref(),
            account: &caller.account,
            acting_for: caller.acting_for(),
            reach: &reach,
            declared: &declared,
        };
        if let Some(refusal) = checks::refuse(&core, &who, &request).await {
            let id = Value::String(request.id.as_str().to_string());
            if !send(&out, wire::error(id, refusal, locale)).await {
                break;
            }
            continue;
        }
        if peer.is_some()
            && core.serves(&request.method)
            && let Some(handler) = core.queries.background(&request.method)
        {
            background.spawn(answer_later(
                handler,
                Arc::clone(&core),
                request,
                locale,
                out.clone(),
            ));
            continue;
        }
        if let Some(peer) = peer
            && methods::answered_later(&core, &request)
        {
            let later = subscriptions.later();
            background.spawn(call_later(
                Arc::clone(&core),
                peer,
                caller.clone(),
                request,
                later,
                out.clone(),
            ));
            continue;
        }
        let id = || Value::String(request.id.as_str().to_string());
        let (answer, target, close) = match (request.method.as_str(), peer) {
            ("hello", _) => {
                match hello(&core, request.params.clone(), from_core, &caller.account).await {
                    Ok((shook, by, mut result)) => {
                        reach::strip(&reach, &mut result);
                        if let Some((ready, _)) = spawned.take()
                            && ready.send(()).is_err()
                        {
                            // 看管它的那一头不等了：不用说。
                        }
                        if let Some(package) = &package {
                            let given = Handing::now(&core, package);
                            result["config"] = json!(given.handed);
                            handing = Some(given);
                        }
                        shaken = Some(shook);
                        revoked = (by.login().is_some() || core.setting_up())
                            .then(|| core.identity.revoked());
                        // 凭据认出来的账号就是这个连接（施工 Y-1 上）。
                        if let Some(account) = by.account() {
                            caller.account = account.clone();
                        }
                        via = Some(by);
                        (wire::result(&request.id, result), None, false)
                    }
                    // 被拒的，话照这一次报的语言说（施工 4-9 再补三上）：第一次握手也不是一律英文。
                    Err((refusal, close)) => {
                        let asked = asked_locale(&request.params).unwrap_or(locale);
                        (wire::error(id(), refusal, asked), None, close)
                    }
                }
            }
            (_, None) => (wire::error(id(), Refusal::HELLO_FIRST, locale), None, false),
            ("subscribe", Some(peer)) => {
                let (result, target) = streams::subscribe(
                    &core,
                    &mut subscriptions,
                    &request,
                    (shaken, &caller),
                    &out,
                )
                .await;
                (answer(&request, result, peer.locale), target, false)
            }
            (name, Some(_)) if name.starts_with("login.") => {
                let result =
                    login::ticket::call(&core, via.as_ref(), &caller.account, &request).await;
                (answer(&request, result, locale), None, false)
            }
            (name, Some(_)) if name.starts_with("account.") => {
                let changed = accounts::call(&core, via.as_ref(), &caller.account, &request).await;
                if let Some(by) = changed.via {
                    revoked = Some(core.identity.revoked());
                    if let Some(account) = by.account() {
                        caller.account = account.clone();
                    }
                    via = Some(by);
                }
                (
                    answer(&request, changed.result, locale),
                    None,
                    changed.close,
                )
            }
            ("unsubscribe", Some(_)) => {
                let result = stream_of(&request).map(|stream| {
                    match stream {
                        Stream::Events(session) | Stream::View(session) => {
                            subscriptions.remove(&session);
                        }
                        Stream::Config => subscriptions.remove_config(),
                        Stream::Sessions => subscriptions.remove_sessions(),
                        Stream::Extensions => subscriptions.remove_listed(false),
                        Stream::Packages => subscriptions.remove_listed(true),
                        Stream::Projects => subscriptions.remove_projects(),
                        Stream::Knowledge => subscriptions.remove_kb(),
                        Stream::Memory(at) => subscriptions.remove_memory(&at),
                        Stream::Widgets(_) => subscriptions.remove_widgets(),
                    }
                    json!({})
                });
                (answer(&request, result, locale), None, false)
            }
            (_, Some(peer)) => {
                let result = methods::call(&core, peer, &caller, &request, &mut uploads).await;
                (answer(&request, result, locale), target(&request), false)
            }
        };
        // 握手的回应写出去了，才起推包自己的配置的任务（施工 9-4 下下）：推送不会跑到回应前面。
        let replied = subscriptions.reply(target.as_ref(), answer, &out).await;
        if let Some(given) = handing.take() {
            subscriptions.add_extension_config(
                &core,
                given.package,
                given.handed,
                given.current,
                &out,
            );
        }
        if !replied || close {
            break;
        }
    }
    // 连接断了：在后台答的一起停（施工 W-7）；它开着的上传全部作废，删暂存文件（`web-module.md`「怎么走」第六条
    // 第 4 款）。
    background.abort_all();
    uploads.discard_all().await;
    // 在等它回的反向调用都了结；它提供的工具从此暂时不可用（施工 O-2 上）：提供者表里留着的这一头一叫就了结，重新登记的换掉它。
    reverse.close();
    if shaken.is_some() {
        tracing::info!(target: "miyu::endpoint", "disconnected");
    }
}

/// 等到作废了这个连接靠的登录令牌 `mine`（施工 W-8）：这个账号 `account` 的全部作废（施工 Y-1 上起只断这个账号的）、作废的
/// 正是它；收慢了丢了几条的也当作废了（页面照登录令牌重连，还认得的照常进来）。
async fn until_revoked(
    revoked: &mut tokio::sync::broadcast::Receiver<Revoked>,
    mine: &str,
    account: &miyu_kernel::id::AccountId,
) {
    loop {
        match revoked.recv().await {
            Ok(Revoked::One(hash)) if hash != mine => {}
            Ok(Revoked::All(whose)) if whose != *account => {}
            Ok(_) | Err(_) => return,
        }
    }
}

/// 在后台答一条（施工 W-7）：办完了把回应放进写队列。连接已经断了的，放不进去也不要紧，没人收了。
async fn answer_later(
    handler: Handler,
    core: Arc<Core>,
    request: Request,
    locale: Locale,
    out: mpsc::Sender<String>,
) {
    let result = handler(core, request.params.clone())
        .await
        .map_err(Refusal::from);
    send(&out, answer(&request, result, locale)).await;
}

/// 在后台办一条自带的请求（施工 8-20 补、R-7 补，[`methods::answered_later`]）：办完了把回应照 `peer` 的语言写出去，经
/// `later` 照那一刻的订阅走（给会话的排在它已经到了的推送后面，「先见结果，后见回应」）。
async fn call_later(
    core: Arc<Core>,
    peer: crate::hello::Peer,
    caller: Caller,
    request: Request,
    later: Later,
    out: mpsc::Sender<String>,
) {
    let result = methods::call_later(&core, peer, &caller, &request).await;
    let line = answer(&request, result, peer.locale);
    later.reply(target(&request).as_ref(), line, &out).await;
}

/// 收掉办完了的后台任务，不让这一组越攒越多；崩了的记一行（它的回应永远不会来了）。
fn reap(background: &mut JoinSet<()>) {
    while let Some(joined) = background.try_join_next() {
        if let Err(error) = joined
            && error.is_panic()
        {
            tracing::error!(target: "miyu::endpoint", panic = error.is_panic(), "background request panicked");
        }
    }
}

/// `hello` 里报的语言，读得出来的话（施工 4-9 再补三上）：握手被拒时照它说。
fn asked_locale(params: &Value) -> Option<Locale> {
    params
        .get("locale")
        .and_then(Value::as_str)
        .map(|locale| Locale::of(Some(locale)))
}

/// 命令的回应经哪个订阅写出去：给会话的经这个会话的订阅，`config.set` 经配置的订阅（施工 8-4）。
fn target(request: &Request) -> Option<Target> {
    if request.method == "config.set" {
        return Some(Target::Config);
    }
    request
        .params
        .get("session")
        .and_then(Value::as_str)
        .and_then(|session| SessionId::parse(session).ok())
        .map(Target::Session)
}

/// 回应写成一行：接受的 `result`，拒绝的 `error`。
fn answer(request: &Request, result: Result<Value, Refusal>, locale: Locale) -> String {
    match result {
        Ok(result) => wire::result(&request.id, result),
        Err(refusal) => wire::error(
            Value::String(request.id.as_str().to_string()),
            refusal,
            locale,
        ),
    }
}

/// 放进写队列；写队列关了（连接断了），交回 `false`。
async fn send(out: &mpsc::Sender<String>, line: String) -> bool {
    out.send(line).await.is_ok()
}

/// 握手时交给核心拉起的扩展的那份（施工 9-4 下下）：包、交出去的键到值、握手时拿的盯配置的那一头。
struct Handing {
    package: String,
    handed: std::collections::BTreeMap<String, Value>,
    current: tokio::sync::watch::Receiver<Arc<crate::config::Config>>,
}

impl Handing {
    /// 照这一刻的配置算包 `package` 的那份，盯配置的那一头记成看过这一份。
    fn now(core: &Core, package: &str) -> Handing {
        let mut current = core.config_now();
        let config = Arc::clone(&current.borrow_and_update());
        let handed = crate::extensions::config::own(&config, package);
        tracing::debug!(target: "miyu::endpoint", package, keys = handed.len(), "extension config handed");
        Handing {
            package: package.to_string(),
            handed,
            current,
        }
    }
}
