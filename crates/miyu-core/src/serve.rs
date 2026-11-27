//! 一个个接连接，空闲够久了、收到停的信号就停（`internal/designs/12-进程形态与分发.md` 第二节「按需运行」，
//! 施工 3-9 上）。施工 Z-1 上起默认常驻：空闲退不退照系统配置的 `core.idle_exit`（[`CoreSettings`]，分钟，0 是不退），每次看空闲
//! 时现取，改了当场算数。停的信号怎么装、收尾的总时限在 `stops.rs`（施工 CS-6）。

use std::sync::Arc;
use std::time::{Duration, Instant};

use miyu_endpoint::Core;
use miyu_ipc::Listener;

use crate::TARGET;

miyu_config::settings! {
    /// 核心进程的配置（施工 Z-1 上；`workers` 施工 CPU-1）。
    pub struct CoreSettings in "core" {
        /// 空闲多少分钟以后退出：没有连接、也没有在跑的回合，连续这么久。0 是不退（默认）。只在系统配置里。
        idle_exit: i64 = 0 {
            kind: int [0, 525600],
            layers: [System],
            applies: now,
            ui: { page: "advanced", group: "core", control: number },
        },
        /// 运行时开几个工作线程（施工 CPU-1）：0 是照 CPU 核数（默认），写了的最少 2 个；超过 64 的照写坏了报、照核数。
        /// 只在系统配置里，建运行时以前读一次（`workers.rs`），改了重启核心生效。
        workers: i64 = 0 {
            kind: int [0, 64],
            layers: [System],
            applies: restart,
            ui: { page: "advanced", group: "core", control: number },
        },
    }
}

/// 照核心这一刻的配置交回空闲时限：`core.idle_exit` 是 0 的没有（不退）。
pub fn idle_limit(core: &Core) -> Option<Duration> {
    let config = Arc::clone(&core.config_now().borrow());
    let minutes = CoreSettings::from(&config.resolved().values()).idle_exit;
    u64::try_from(minutes)
        .ok()
        .filter(|minutes| *minutes > 0)
        .map(|minutes| Duration::from_secs(minutes * 60))
}

/// 没有空闲时限时多久再看一次配置：时限改了以后最多这么久算上。
const RELOOK: Duration = Duration::from_secs(30);

/// 为什么停了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// 没有连接、也没有在跑的回合，空闲够久了。
    Idle,
    /// 收到停的信号：在跑的会话先有计划地停下了。
    Signal,
    /// 数据根没了（施工 TMP-2）：锁文件不在了、换成了别的，在跑的会话先有计划地停下了。
    Gone,
    /// 头叫停的（施工 DT-4 下，`core.stop`）：在跑的会话先有计划地停下了，同收到停的信号。
    Request,
}

impl Stopped {
    /// 记进运行日志的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Stopped::Idle => "idle",
            Stopped::Signal => "signal",
            Stopped::Gone => "gone",
            Stopped::Request => "request",
        }
    }
}

/// 接连接，直到空闲够久了（没有连接、也没有在跑的回合，连续 `idle()` 交回的这么久；交回没有的不退），或者 `stop` 来了、头叫停了
/// （`core.stop`，施工 DT-4 下）。这两种都先让在跑的会话有计划地停下：跑到一半的回合记成「重启了」，下次载入接着干。停之前请扩展退出（施工 9-4 上）。
///
/// 连着头的时候不定时看空不空闲（施工 V-6 下）：有连接就不算空闲，等连上、断开、来请求的信号（`Activity`）再算。没有连接
/// 以后才每隔一会儿（`check`：空闲时限的四分之一）看一次（回合、后台命令、扩展忙完没有，没有统一的信号），看到退出为止。
/// 每次看的时候先看数据根里的锁文件还是不是拿着的这一个（施工 TMP-2）：不是了的有计划地停下，交回 [`Stopped::Gone`]。
///
/// 停之前记一条「停了」：那时锁还在手里，下一个核心起来之前，这一份日志还是它一个人写。再关上数据根里的库（施工 CS-9），
/// 才放锁。交回以后，监听器、锁、各个连接、开着的库都没了。
pub async fn serve(
    listener: Listener,
    core: Arc<Core>,
    idle: impl Fn() -> Option<Duration>,
    stop: impl Future<Output = ()>,
) -> Stopped {
    serve_then(listener, core, idle, stop, async {}).await
}

/// 同 [`serve`]，停下以后、放锁之前先做完收尾 `closing`（施工 TMP-1 补2）：清回收处、清缓存、等另起线程写盘的写完，这些都碰
/// 数据根，只许拿着锁的核心做。原来锁一放才等它们，下一份核心起来、测试删数据根时撞上。记忆在后台的读写在收尾之前等完（施工
/// TMP-1 补3）。
pub async fn serve_then(
    listener: Listener,
    core: Arc<Core>,
    idle: impl Fn() -> Option<Duration>,
    stop: impl Future<Output = ()>,
    closing: impl Future<Output = ()>,
) -> Stopped {
    serve_watched(listener, core, idle, (stop, || {}), closing).await
}

/// 同 [`serve_then`]，定下要停的那一刻叫一次 `stopping`（施工 CS-6：核心照它给收尾起表，`stops.rs`）。
///
/// 定下要停以后先不再接新连接（施工 CS-6）：关掉监听、删掉套接字文件和 `run/socket`，头再连是「没在跑」，不会连上了却等不到回应；
/// 已经连着的照常说话，到交回时才断。锁另外拿着，收尾做完、记下「停了」才放。
pub(crate) async fn serve_watched(
    mut listener: Listener,
    core: Arc<Core>,
    idle: impl Fn() -> Option<Duration>,
    (stop, stopping): (impl Future<Output = ()>, impl FnOnce()),
    closing: impl Future<Output = ()>,
) -> Stopped {
    // 数据根里的锁文件还是不是拿着的这一个（施工 TMP-2）：监听器交出去以前拿好。锁也拿出来：监听器先放（施工 CS-6）。
    let held = listener.held();
    let lock = listener.take_lock();
    let (close, closed) = tokio::sync::oneshot::channel::<()>();
    // 放在堆上：交回以前先放下它（连接都断），再放锁。
    let mut running = Box::pin(miyu_endpoint::run_until(
        listener,
        Arc::clone(&core),
        closed,
    ));
    let activity = core.activity();
    // 头叫停（施工 DT-4 下，`core.stop`）：同收到停的信号，只是原因不一样。
    let asked = activity.stop_requested();
    tokio::pin!(stop, asked);
    let mut changes = activity.subscribe();
    let every = idle().map_or(RELOOK, check);
    // 下一次看的时刻：定死了，有动静不往后推（一直有人来连一下就走的，照样到点看）。
    let mut next = tokio::time::Instant::now() + every;
    let mut quiet: Option<Instant> = None;
    let stopped = loop {
        // 连着头：不算空闲，空闲从断开以后重新算。
        let connected = core.connections() > 0;
        if connected {
            quiet = None;
        }
        tokio::select! {
            never = &mut running => match never {},
            () = &mut stop => break Stopped::Signal,
            () = &mut asked => break Stopped::Request,
            // 有动静了（连上、断开、来请求）：回到上面重新看。信号断不了（核心在它就在），断了也当没动静。
            Ok(()) = changes.changed() => continue,
            () = tokio::time::sleep_until(next), if !connected => {
                next = tokio::time::Instant::now() + every;
                // 数据根没了（施工 TMP-2）：测试删了临时目录、人手删了的，锁文件不在了、换成了别的。不走的话，没有空闲时限的
                // 核心永远开着，新建的数据根上还能再起一个。
                if !held.here() {
                    tracing::warn!(target: TARGET, "data root gone, stopping");
                    break Stopped::Gone;
                }
                if !miyu_kernel::apart::future(core.idle()).await {
                    quiet = None;
                    continue;
                }
                let since = *quiet.get_or_insert_with(Instant::now);
                if idle().is_some_and(|idle| since.elapsed() >= idle) {
                    break Stopped::Idle;
                }
            }
        }
    };
    stopping();
    // 不再接新连接（施工 CS-6）：接连接的那一头收到就关监听。
    let _closed = close.send(());
    // 停下以后的收尾只走一次，单拎出去（施工 LW-8，`miyu_kernel::apart`）。连着的头照常说话：收尾时也接着跑接连接的那一头。
    let stopping_work = miyu_kernel::apart::future(async {
        // 空闲退出的没有在跑的会话；别的先有计划地停下会话。
        if stopped != Stopped::Idle {
            core.stop_sessions().await;
        }
        // 扩展随核心退出（施工 9-4 上）：关它们的标准输入，一起等，没退的杀掉。
        core.stop_extensions().await;
        // 记忆在后台的读写做完（施工 TMP-1 补3）：空闲退出的没停会话，这里等；别的停会话时等过了（`stop_sessions`）。
        if stopped == Stopped::Idle {
            core.finish_writes().await;
        }
    });
    // 收尾在放锁之前（施工 TMP-1 补2）。
    let shutdown = async {
        stopping_work.await;
        closing.await;
    };
    tokio::select! {
        never = &mut running => match never {},
        () = shutdown => {}
    }
    tracing::info!(target: TARGET, reason = stopped.as_str(), "stopped");
    drop(running);
    // 库在放锁以前关上（施工 CS-9）：连接都断了、收尾都做完了，之后这一份核心不再碰数据根里的库文件。关的时候不合 `-wal`。
    core.close_databases().await;
    drop(lock);
    stopped
}

/// 多久看一次空不空闲：空闲时限的四分之一，最多 30 秒，最少 100 毫秒。不另起忙循环。
fn check(idle: Duration) -> Duration {
    (idle / 4).clamp(Duration::from_millis(100), Duration::from_secs(30))
}

#[cfg(test)]
mod tests;
