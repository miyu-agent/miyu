//! 不靠网页打开软件后台（施工 BP-3，`package-pages.md`「不靠网页打开软件后台」第 3 条，2026-11-09 项目主人定）：目录里的后台页由
//! 核心按需起的本机服务给。`package.page` 第一次要的时候起，只听 `127.0.0.1` 上系统给的端口；闲够了（没有请求、也没有开着的
//! 推送流）就关，关了票全作废，下一次要再起。
//!
//! - 票：每要一次发一张，32 个随机字节，认一个包、记着要的那个连接的语言；服务开着就一直有效。
//! - 宿主页经核心进程里的一条连接调核心（`link.rs`）：同一套握手、同一套权限，不另开口子；一张票一条，用到时才连。
//! - 路由、响应头在 `serve.rs`，宿主页替框答的几样在 `bridge.rs`，宿主页本身在资源目录的 `core/backstage/`。

mod bridge;
mod events;
mod link;
mod serve;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::OnceCell;

use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

const TARGET: &str = "miyu::endpoint";

/// 出厂闲多久就关：十分钟。
const IDLE: Duration = Duration::from_secs(600);

/// 核心按需起的后台页服务：没起、关了的没有。
pub(crate) struct Host {
    running: tokio::sync::Mutex<Option<Arc<Running>>>,
    idle: Duration,
}

impl Default for Host {
    fn default() -> Host {
        Host::with_idle(IDLE)
    }
}

impl Host {
    /// 闲了 `idle` 就关（测试里设短的）。
    pub(crate) fn with_idle(idle: Duration) -> Host {
        Host {
            running: tokio::sync::Mutex::new(None),
            idle,
        }
    }
}

/// 开着的服务：听的端口、发出去的票、最近一次请求、开着几条推送流。
pub(super) struct Running {
    port: u16,
    tickets: Mutex<HashMap<String, Arc<Ticket>>>,
    last: Mutex<Instant>,
    streams: AtomicUsize,
    closed: AtomicBool,
    core: Weak<Core>,
    /// 推送流多久推一行注释（`events.rs`）：不超过闲置期的一半，断了的推送流在关服务之前就认出来。
    beat: Duration,
}

impl Running {
    /// 票对得上的那一张。
    fn ticket(&self, value: &str) -> Option<Arc<Ticket>> {
        let tickets = self.tickets.lock().unwrap_or_else(PoisonError::into_inner);
        tickets.get(value).cloned()
    }

    /// 来了一个请求。
    fn touch(&self) {
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
    }

    /// 闲够了：没有开着的推送流，最近一次请求过了 `idle`。
    fn idle(&self, idle: Duration) -> bool {
        self.streams.load(Ordering::SeqCst) == 0
            && self
                .last
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .elapsed()
                >= idle
    }

    /// 关：票全作废，连核心的那几条跟着断。
    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.tickets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

/// 一张票：认的包、要的那个连接的语言、宿主页调核心的那条连接（用到时才连）。
pub(super) struct Ticket {
    package: String,
    language: String,
    link: OnceCell<Arc<link::Link>>,
}

impl Ticket {
    /// 宿主页调核心的那条连接：第一次用时连上、握手。
    async fn link(&self, core: &Arc<Core>) -> Result<Arc<link::Link>, Refusal> {
        self.link
            .get_or_try_init(|| link::Link::open(Arc::clone(core)))
            .await
            .cloned()
    }
}

/// `package.page` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PageParams {
    package: String,
}

/// `package.page`：`{"url"}`，宿主页的地址。没有这个包、没装的 `unknown_package`，没有目录里的后台页的 `no_page`。
pub(crate) async fn open(
    core: &Arc<Core>,
    peer: Peer,
    params: PageParams,
) -> Result<Value, Refusal> {
    super::file::page_dir(core, &params.package)?;
    let running = start(core).await?;
    let value = crate::login::random_hex()?;
    let ticket = Ticket {
        package: params.package,
        language: peer.language.to_string(),
        link: OnceCell::new(),
    };
    running
        .tickets
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(value.clone(), Arc::new(ticket));
    running.touch();
    Ok(json!({"url": format!("http://127.0.0.1:{}/h/{value}/", running.port)}))
}

/// 开着的服务：没起、关了的现在起。
async fn start(core: &Arc<Core>) -> Result<Arc<Running>, Refusal> {
    let host = &core.backstage.host;
    let mut running = host.running.lock().await;
    if let Some(open) = running
        .as_ref()
        .filter(|open| !open.closed.load(Ordering::SeqCst))
    {
        return Ok(Arc::clone(open));
    }
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|error| {
            tracing::warn!(target: TARGET, kind = ?error.kind(), "page host not started");
            Refusal::INTERNAL
        })?;
    let port = listener.local_addr().map_err(|_| Refusal::INTERNAL)?.port();
    let open = Arc::new(Running {
        port,
        tickets: Mutex::new(HashMap::new()),
        last: Mutex::new(Instant::now()),
        streams: AtomicUsize::new(0),
        closed: AtomicBool::new(false),
        core: Arc::downgrade(core),
        beat: (host.idle / 2).min(events::BEAT),
    });
    tracing::info!(target: TARGET, port, "page host started");
    tokio::spawn(serve::run(listener, Arc::clone(&open), host.idle));
    *running = Some(Arc::clone(&open));
    Ok(open)
}
