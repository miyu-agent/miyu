//! 造会话（施工 CS-9 从 `sessions.rs` 挪出来）：照命令编号去重，找好预设、人格，交给 `miyu_session` 造，放进会话表。
//!
//! 一直拿着会话表的锁：同一个命令编号重发的两条只造一个（`04-核心协议.md` 第六节第 1 条）。核心要停了的（`sessions/stopping.rs`）
//! 不拿着锁等：读的那几步当场不读了，交给 `miyu_session` 的那一段另起一个任务，这里交回 `session_stopped`、放开锁，写到一半的
//! 由停的一方等完、停下（2026-11-27 复现：盘慢时建会话拿着锁几十秒，收尾停会话一直等它，SIGTERM 以后 17.6 秒才停下）。

use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_session::new_id;
use miyu_store::log::first_event;

use super::stopping::Made;
use super::{Created, Heaped, Opening, Running, Sessions, Setup, check_dirs, now, opening_cwd};
use crate::Core;
use crate::refusal::Refusal;

/// 记住最近多少个造会话的命令编号：断线重发的造会话不再造一个新的（`04-核心协议.md` 第六节第 1 条）。核心重启以后
/// 第一次造会话时，从最新的这么多个会话的 `session.created` 里补回来（施工 4-9 再补三上）。
const REMEMBERED: usize = 1024;

impl Sessions {
    /// 造一个会话：有没有人能确认照 `attended`；`miyu ask` 开的是一次性的。先找预设（`who.preset`，
    /// 没写的照默认预设，施工 P-2 上，`presets.rs`），再找人格：`persona` 照 [`crate::personas::resolve`]，预设不再参与（施工 P-4
    /// 上，2026-10-08 项目主人：只去掉预设的「默认人格」）。同一个命令编号重发，交回上一次造的那一个，预设、人格都不再找。
    /// 核心要停了的交回 `session_stopped`（施工 CS-9）。
    pub(crate) fn create<'a>(
        &'a self,
        core: &'a Arc<Core>,
        command: CommandId,
        persona: Option<Option<&'a str>>,
        cwd: String,
        dirs: Vec<String>,
        who: Opening,
    ) -> Heaped<'a, Result<Created, Refusal>> {
        Box::pin(async move {
            check_dirs(core, &dirs)?;
            let mut open = self.open.lock().await;
            if self.stopping.now() {
                return Err(Refusal::STOPPED);
            }
            if !open.recalled {
                let earlier = self
                    .stopping
                    .unless(recall(core))
                    .await
                    .ok_or(Refusal::STOPPED)?;
                open.recalled = true;
                for (made, session) in earlier.into_iter().rev() {
                    open.created.push_front((made, session, None));
                }
                while open.created.len() > REMEMBERED {
                    open.created.pop_front();
                }
            }
            // 同一个命令编号重发的，交回上一次造的那一个；工作目录照它的默认工作区挑（施工 5-14）。除了 `cwd`、`dirs` 别的参数换了的
            // 拒（施工 YC-7）：没有指纹的（重启以后补回来的）照旧交回。
            let made = command.clone();
            let digest = who.digest(persona);
            let earlier = match open.created.iter().find(|(seen, ..)| *seen == made) {
                Some((_, _, Some(seen))) if *seen != digest => return Err(Refusal::ID_REUSED),
                Some((_, session, _)) => Some(session.clone()),
                None => None,
            };
            let id = earlier.clone().unwrap_or_else(|| new_id(now()));
            let (workspace, wide) = opening_cwd(core, &who, &cwd, &id);
            // 开局只读照这个会话实际干活的目录算，带上信任着的项目配置（`config.md` 第二条第 9 条）。
            let (_, project) = core.config().with_project(&workspace);
            let untrusted = project.and_then(|project| project.untrusted());
            if let Some(id) = earlier {
                return Ok(Created {
                    id,
                    cwd: workspace,
                    wide,
                    untrusted,
                });
            }
            let resolving = Setup::resolve(
                core,
                (id.clone(), command.clone()),
                persona,
                (workspace.clone(), dirs.clone()),
                who,
            );
            let setup = self
                .stopping
                .unless(resolving)
                .await
                .ok_or(Refusal::STOPPED)??;
            let handle = match self.stopping.make(core, setup).await {
                Made::Done(handle) => handle,
                Made::Stopped => return Err(Refusal::STOPPED),
                // 人格已经找好了：`core/` 下哪一份读不了（安装坏了），是内部出错（施工 4-9 再补三上）。
                Made::Failed(error) => {
                    tracing::warn!(target: "miyu::endpoint", error = %error, "create failed");
                    return Err(Refusal::INTERNAL);
                }
                Made::Panicked => {
                    tracing::error!(target: "miyu::endpoint", "create panicked");
                    return Err(Refusal::INTERNAL);
                }
            };
            open.running.insert(
                id.clone(),
                Running {
                    handle,
                    workspace: workspace.clone(),
                    dirs,
                },
            );
            open.created.push_back((made, id.clone(), Some(digest)));
            if open.created.len() > REMEMBERED {
                open.created.pop_front();
            }
            // 内存紧的话先腾地方（施工 MEM-2）：只叫一声，不等、不拒。
            self.opened();
            self.sweep(core);
            Ok(Created {
                id,
                cwd: workspace,
                wide,
                untrusted,
            })
        })
    }
}

/// 核心重启以后补回去重的编号（施工 4-9 再补三上）：最新的 [`REMEMBERED`] 个会话（会话编号是 UUIDv7，[`miyu_store::root::DataRoot::sessions`]
/// 已经从新到旧），`session.created` 的 `cause` 就是造会话的命令编号。读不了的跳过（造到一半、只有空的第一段的会话也是：
/// 它没有 `session.created`）。在阻塞线程里读，交回的照从旧到新。
async fn recall(core: &Core) -> Vec<(CommandId, SessionId)> {
    let root = core.root.clone();
    tokio::task::spawn_blocking(move || {
        let mut found: Vec<(CommandId, SessionId)> = root
            .sessions()
            .unwrap_or_default()
            .into_iter()
            .take(REMEMBERED)
            .filter_map(|id| {
                let event = first_event(&root.session_dir(&id)).ok()?;
                match (&event.body, event.cause) {
                    (Body::SessionCreated(_), Some(cause)) => Some((cause, id)),
                    _ => None,
                }
            })
            .collect();
        found.reverse();
        found
    })
    .await
    .unwrap_or_default()
}
