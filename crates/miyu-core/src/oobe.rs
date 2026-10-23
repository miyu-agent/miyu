//! 机主和设置那一段（施工 Y-12 中上，设计 06 U22）：`admin` 是系统、不登录，装 Miyu 的那个人 OOBE 时注册自己的账号，核心照他
//! 起（会话、记忆、用量、个人设置、包都在他的家目录）。
//!
//! 起来时照 `system/accounts.json` 找机主（[`owner`]）。没有的先跑设置那一段（[`setup`]）：一份只能设置的核心（本机令牌的连接只能
//! 握手、注册机主、要一次性码），不拉扩展、不起任何监视；注册成了，等这一段的连接都断开，收掉它，在同一个监听器、同一把锁上
//! 照机主起完整的那一份。头连不上的空档没有，后台任务也不会留下两份。

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use miyu_endpoint::Core;
use miyu_ipc::Listener;
use miyu_kernel::id::AccountId;
use miyu_store::accounts;
use miyu_store::env::Env;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::TARGET;

/// 注册了机主以后，最多等设置那一段的连接断开多久：回应写出去、各自断开，一般几毫秒。
const DRAIN: Duration = Duration::from_secs(2);

/// 接不了连接以后歇多久再接（同端点的 `listen.rs`）。
const LOOK: Duration = Duration::from_millis(100);

/// 机主：账号文件里标了 `owner`、没停用的那一个。没有这个文件、读不了、还没 OOBE 的没有（读不了的记一行）。
pub fn owner(root: &DataRoot) -> Option<AccountId> {
    let path = root.system().join(accounts::FILE);
    let read = accounts::read(&path)
        .inspect_err(|error| tracing::warn!(target: TARGET, error = %error, "accounts not read"))
        .ok()?;
    let owner = read.accounts.owner()?;
    AccountId::parse(&owner.id)
        .inspect_err(|_| tracing::warn!(target: TARGET, "owner id not usable"))
        .ok()
}

/// 设置那一段：在 `listener` 上接连接，交给只能设置的一份核心，直到注册了机主（交回他）、`stop` 来了，或者没有连接满 `idle`
/// 这么久（同完整的那一份的空闲退出，没有的不退；后两种交回没有，施工 Y-12 中上修）。`token` 是这一次的本机令牌。
///
/// # Errors
///
/// 供应商的档案读不出、写法不对：造不出路由，这一份核心起不来。
pub async fn setup(
    (env, root, resources): (&Env, &DataRoot, &ResourceRoot),
    (listener, token): (&mut Listener, &str),
    (stop, idle): (impl Future<Output = ()>, Option<Duration>),
) -> Result<Option<AccountId>, String> {
    let routes = crate::models::prepare(resources, Some(root.state().join("models")))?;
    let core = Arc::new(
        Core::new(
            root.clone(),
            resources.clone(),
            Arc::new(routes),
            Catalog::default(),
            env.home.clone(),
            crate::admin(),
            token.to_string(),
        )
        .in_setup(),
    );
    let done = core
        .setup_done()
        .ok_or_else(|| "setup core has no setup".to_string())?;
    tracing::info!(target: TARGET, "waiting for the owner");
    // 连接另放一组，不跟着接连接的循环走：注册了机主就不再接新的（留在套接字的队列里给完整的那一份接），已经连着的照常写完
    // 回应、各自断开。
    let mut connections = JoinSet::new();
    tokio::pin!(stop);
    // 从什么时候起一个连接都没有：空闲照它算。
    let mut quiet = Instant::now();
    loop {
        let deadline = idle.map(|idle| quiet + idle);
        tokio::select! {
            // 先看停、看注册了没有（`biased`）：注册成了的那个头紧接着就重连，那时接连接、那一响都就绪了，随机挑会让它连进这一份。
            biased;
            () = &mut stop => return Ok(None),
            () = done.notified() => break,
            Some(_) = connections.join_next() => {
                if connections.is_empty() {
                    quiet = Instant::now();
                }
            }
            () = sleep_until(deadline), if deadline.is_some() && connections.is_empty() => {
                tracing::info!(target: TARGET, reason = "idle", "stopped");
                return Ok(None);
            }
            accepted = listener.accept() => match accepted {
                Ok(connection) => {
                    connections.spawn(miyu_endpoint::serve(connection, Arc::clone(&core)));
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, error = %error, "accept failed");
                    tokio::time::sleep(LOOK).await;
                }
            },
        }
    }
    // 回应写出去、连接各自断开以后再收掉这一份：到点还没断的掐掉。
    let drained = async { while connections.join_next().await.is_some() {} };
    if tokio::time::timeout(DRAIN, drained).await.is_err() {
        connections.abort_all();
    }
    let owner = owner(root);
    tracing::info!(target: TARGET, found = owner.is_some(), "setup done");
    Ok(owner)
}

/// 睡到 `deadline`；没有的永远不醒（`select!` 里另有条件挡着，不会走到）。
async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}
