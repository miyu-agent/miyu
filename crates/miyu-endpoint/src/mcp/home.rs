//! 管理员个人设置里的 MCP 服务（施工 F-9 下，`docs/blueprint/mcp.md`「个人设置里的一张表」第 4 到 7 条）：挂在 `~home` 下，
//! 写进去就算开着，盖住包里同名的（包里那个不起、它的功能不算）。核心起来时和包里带的一起起；配置服务换上新的一份（个人设置
//! 手改了被看到）时照新写的换：新写的起，删掉的停下、工具出目录、包里被它盖住的照包的开关起回来，改了的停下再起。密钥变了的，
//! 引用它的服务停下再起（施工 X-32 补，`secrets.rs`）。

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::task::JoinHandle;

use miyu_config::package::{McpServer, PackageKind, Reach, personal_mcp};
use miyu_store::scripts::HOME_PACKAGE;

use super::TARGET;
use crate::Core;

/// 这一刻照着起的服务，照个人设置里写的先后。
#[derive(Default)]
pub(crate) struct HomeMcp(Mutex<Arc<Vec<McpServer>>>);

impl HomeMcp {
    /// 这一刻的。
    pub(crate) fn now(&self) -> Arc<Vec<McpServer>> {
        Arc::clone(&self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// 叫 `service` 的服务写在个人设置里：包里同名的被它盖住。
    pub(crate) fn covers(&self, service: &str) -> bool {
        self.now().iter().any(|server| server.name == service)
    }

    /// 换成 `servers`，交回原来的。
    fn replace(&self, servers: Vec<McpServer>) -> Arc<Vec<McpServer>> {
        std::mem::replace(
            &mut *self.0.lock().unwrap_or_else(PoisonError::into_inner),
            Arc::new(servers),
        )
    }
}

impl Core {
    /// 照手里的个人设置定下有哪几个服务（核心起来时，起包里带的以前：先定下谁盖住谁）。
    pub(crate) fn note_home_mcp(&self) {
        let text = self.config().personal.text.clone();
        self.home_mcp.replace(read(&text));
    }

    /// 起个人设置里的服务：在跑、在等的不动。
    pub(crate) fn start_home_mcp(self: &Arc<Self>) {
        let dir = home_dir(self);
        for server in self.home_mcp.now().iter() {
            self.mcp
                .launch_one(self, HOME_PACKAGE, &tilde(server, &dir), &dir);
        }
    }

    /// 跟着配置换：配置服务每换上一份新的，照个人设置的字换上，只动变了的；密钥变了的，引用它的服务停下再起（施工 X-32 补）。
    /// 开始时先照手里的这一份对一遍：起来和开始跟着之间的手改也认得。核心没了就停。
    pub fn follow_home_mcp(self: &Arc<Self>) -> JoinHandle<()> {
        let mut config = self.config_now();
        let core = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut before = None;
            loop {
                let (text, prints) = {
                    let now = config.borrow_and_update();
                    (now.personal.text.clone(), super::secrets::prints(&now))
                };
                let Some(alive) = core.upgrade() else {
                    break;
                };
                alive.apply_home_mcp(read(&text)).await;
                if let Some(before) = before.replace(prints.clone()) {
                    let changed = super::secrets::changed(&before, &prints);
                    if !changed.is_empty() {
                        alive.follow_secrets(&changed).await;
                    }
                }
                drop(alive);
                if config.changed().await.is_err() {
                    break;
                }
            }
        })
    }

    /// 换成 `new`：只动变了的。删掉的、改了的先停下、工具出目录；删掉的那个名字，包里被它盖住的照包的开关起回来；新写的那个
    /// 名字，包里同名的停下、工具出目录；最后起新写的、改了的。
    async fn apply_home_mcp(self: &Arc<Self>, new: Vec<McpServer>) {
        let old = self.home_mcp.now();
        if same_list(&old, &new) {
            return;
        }
        self.home_mcp.replace(new.clone());
        for server in old.iter() {
            let kept = new
                .iter()
                .any(|now| now.name == server.name && same(now, server));
            if !kept {
                self.mcp.halt_one(self, HOME_PACKAGE, &server.name).await;
            }
        }
        for server in old
            .iter()
            .filter(|server| !new.iter().any(|now| now.name == server.name))
        {
            self.uncover(&server.name);
            // 删掉的服务，登录存的那一份连同端口一起清掉（施工 X-42 上）。
            crate::secrets::discard(self, &super::login::secret_name(&server.name));
        }
        for server in new
            .iter()
            .filter(|now| !old.iter().any(|server| server.name == now.name))
        {
            for id in carrying(self, &server.name) {
                self.mcp.halt_one(self, &id, &server.name).await;
            }
        }
        self.start_home_mcp();
        // 盖住、放开了的包，列表上那一项的 `covered` 变了（施工 F-9 补）：服务没起停的也报一声。
        for name in old
            .iter()
            .chain(new.iter())
            .map(|server| server.name.as_str())
            .filter(|name| {
                old.iter().any(|s| s.name == *name) != new.iter().any(|s| s.name == *name)
            })
        {
            for id in carrying(self, name) {
                self.extensions.notify(&id);
            }
        }
        tracing::info!(target: TARGET, servers = new.len(), "mcp settings followed");
    }

    /// 重启个人设置里的服务 `name`（施工 F-9 补，`mcp.restart`）：停下、再起，连续失败从零数；工具留在目录里，起好列完照
    /// 新的换。
    pub(crate) async fn restart_home_mcp(self: &Arc<Self>, name: &str) {
        self.mcp.stop_one(self, HOME_PACKAGE, name).await;
        self.start_home_mcp();
    }

    /// 个人设置里删掉了 `service`：包里带同名服务的，开着（`bundle` 照开关，扩展照它在跑）的起回来。
    fn uncover(self: &Arc<Self>, service: &str) {
        for found in self.packages().iter() {
            let Ok(manifest) = &found.read else {
                continue;
            };
            if !manifest.mcp.iter().any(|server| server.name == service) {
                continue;
            }
            let on = match manifest.kind {
                PackageKind::Bundle => crate::extensions::switched_on(self, &found.id, manifest),
                _ => self.extensions.status(&found.id).state.busy(),
            };
            if on {
                self.start_services(&found.id, manifest);
            }
        }
    }
}

/// 读个人设置的字 `text` 里的服务：写错的各记一行 `WARN mcp settings problem`，不收。
fn read(text: &str) -> Vec<McpServer> {
    let (servers, problems) = personal_mcp(text);
    for problem in &problems {
        tracing::warn!(target: TARGET, line = problem.line, code = problem.code.as_str(), detail = problem.detail.as_str(), "mcp settings problem");
    }
    servers
}

/// 个人设置里的服务 `service` 的状态变了，报给订阅着的连接的那个键：`~home/<服务名>`（施工 F-9 补）。
pub(super) fn changed(service: &str) -> String {
    format!("{HOME_PACKAGE}/{service}")
}

/// 推送的键 `key` 是个人设置里哪个服务的；不是的没有。
pub(crate) fn server_of(key: &str) -> Option<&str> {
    key.strip_prefix(HOME_PACKAGE)?.strip_prefix('/')
}

/// 带叫 `service` 的服务的包的编号。
pub(super) fn carrying(core: &Core, service: &str) -> Vec<String> {
    core.packages()
        .iter()
        .filter(|found| {
            found
                .read
                .as_ref()
                .is_ok_and(|manifest| manifest.mcp.iter().any(|server| server.name == service))
        })
        .map(|found| found.id.clone())
        .collect()
}

/// 程序照哪个目录找：系统的家目录（带路径分隔符的相对路径照它接，只有名字的先找它下面的 `bin/`）；不知道的是空的，只照
/// `PATH` 找。
fn home_dir(core: &Core) -> PathBuf {
    core.home.clone().unwrap_or_default()
}

/// 命令第一项写成 `~/` 开头的，换成家目录 `home` 下的绝对路径；不知道家目录的、网址的照原样。
fn tilde(server: &McpServer, home: &std::path::Path) -> McpServer {
    let mut server = server.clone();
    if let Reach::Command { command, .. } = &mut server.reach
        && let Some(first) = command.first_mut()
        && let Some(rest) = first.strip_prefix("~/")
        && home.is_absolute()
    {
        *first = home.join(rest).to_string_lossy().into_owned();
    }
    server
}

/// 两个服务一样：除了写在第几行，每一格都一样（加一行注释不重起）。
fn same(a: &McpServer, b: &McpServer) -> bool {
    a.name == b.name
        && a.reach == b.reach
        && a.venues == b.venues
        && a.timeout_ms == b.timeout_ms
        && a.need_load == b.need_load
        && a.clicall == b.clicall
}

/// 两份一样：个数、先后、每一个都一样。
fn same_list(a: &[McpServer], b: &[McpServer]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
}

#[cfg(test)]
mod tests;
