//! 装包的进度推给发起的连接（施工 G-4 上起推下载，IN-3 带阶段，设计 `49-装包照pacman.md` 第三节第 4 条）：`package.progress`
//! 带 `stage`（`download`、`verify`、`install`）和这是第几个、一共几个（`index`、`count`，从 1 数）；下载的另带下了多少、一共多少。
//! 她的工具经端口装的没有连接，不推（施工 G-6）。

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::TARGET;
use crate::reverse::Peer as Reverse;

/// 下载进度最多多久推一次。
const EVERY: Duration = Duration::from_millis(200);

/// 装一个包走到了哪一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    /// 下载（照旧带 `done`、`total`）。
    Download,
    /// 核对完整性：SHA-256 照签过名的索引对，包目录、包文件照清单查。
    Verify,
    /// 装进去。
    Install,
}

impl Stage {
    fn as_str(self) -> &'static str {
        match self {
            Stage::Download => "download",
            Stage::Verify => "verify",
            Stage::Install => "install",
        }
    }
}

/// 这一次装的第几个（从 1 数）、一共几个。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct At {
    pub(crate) index: usize,
    pub(crate) count: usize,
}

impl At {
    /// 一共 `count` 个里排第 `zero_based` 个（从 0 数）的那一个。
    pub(crate) fn nth(zero_based: usize, count: usize) -> At {
        At {
            index: zero_based + 1,
            count,
        }
    }

    /// 只装一个（包目录、包文件）。
    pub(crate) const ONLY: At = At { index: 1, count: 1 };
}

/// 推给发起的连接：包 `id` 走到了 `stage`（核对、安装之前各推一次）。没有连接的不推。
pub(crate) fn stage(caller: Option<&Reverse>, id: &str, stage: Stage, at: At) {
    push(
        caller,
        id,
        json!({"package": id, "stage": stage.as_str(), "index": at.index, "count": at.count}),
    );
}

/// 下载包 `id` 的进度：开始、结束各推一次，中间最多 [`EVERY`] 一次。
pub(crate) fn downloading<'a>(
    caller: Option<&'a Reverse>,
    id: &'a str,
    at: At,
) -> impl Fn(u64, u64) + Sync + 'a {
    let last: Mutex<Option<Instant>> = Mutex::new(None);
    move |done, total| {
        if caller.is_none() {
            return;
        }
        let mut last = last.lock().unwrap_or_else(PoisonError::into_inner);
        let due = done == 0 || done == total || last.is_none_or(|at| at.elapsed() >= EVERY);
        if due {
            *last = Some(Instant::now());
            let params = json!({"package": id, "stage": Stage::Download.as_str(), "done": done,
                                "total": total, "index": at.index, "count": at.count});
            push(caller, id, params);
        }
    }
}

fn push(caller: Option<&Reverse>, id: &str, params: Value) {
    let Some(caller) = caller else {
        return;
    };
    if !caller.notify("package.progress", params) {
        tracing::debug!(target: TARGET, package = id, "package progress not delivered");
    }
}
