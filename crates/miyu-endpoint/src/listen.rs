//! 在监听器上一个个接连接，每个连接交给 [`serve`]（施工 3-8 下）。

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use miyu_ipc::Listener;
use tokio::task::JoinSet;

use crate::{Core, serve};

/// 接不了连接以后歇多久再接：打开的文件太多这类错一时好不了，不歇会空转。
const PAUSE: Duration = Duration::from_millis(100);

/// 在 `listener` 上一个个接连接，每个交给 [`serve`]，一直接下去。不会自己停：要停就丢掉它，监听器、
/// 锁、各个连接跟着一起没了。
pub async fn run(listener: Listener, core: Arc<Core>) -> Infallible {
    run_until(listener, core, std::future::pending::<()>()).await
}

/// 同 [`run`]，只是 `close` 来了以后不再接新连接（施工 CS-6，核心定下要停时）：丢掉监听器（关监听、删套接字文件和 `run/socket`，
/// 拿走了锁的不放锁），记一行 `INFO listener closed`；已经连着的照常说话，到丢掉它时才断。
pub async fn run_until(listener: Listener, core: Arc<Core>, close: impl Future) -> Infallible {
    let mut listener = Some(listener);
    let mut connections = JoinSet::new();
    tokio::pin!(close);
    loop {
        tokio::select! {
            accepted = accept(&mut listener) => match accepted {
                Ok(connection) => {
                    tracing::debug!(target: "miyu::endpoint", "connected");
                    connections.spawn(serve(connection, Arc::clone(&core)));
                }
                Err(error) => {
                    tracing::warn!(target: "miyu::endpoint", error = %error, "accept failed");
                    tokio::time::sleep(PAUSE).await;
                }
            },
            _ = &mut close, if listener.is_some() => {
                listener = None;
                tracing::info!(target: "miyu::endpoint", "listener closed");
            }
            Some(done) = connections.join_next() => {
                if let Err(error) = done {
                    tracing::error!(target: "miyu::endpoint", panic = error.is_panic(), "connection task failed");
                }
            }
        }
    }
}

/// 等下一个连接；监听器已经丢了的一直等下去。
async fn accept(listener: &mut Option<Listener>) -> std::io::Result<miyu_ipc::Connection> {
    match listener {
        Some(listener) => listener.accept().await,
        None => std::future::pending().await,
    }
}
