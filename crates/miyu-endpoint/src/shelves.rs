//! 每人一个工具目录架子（施工 U23 中中、中下，设计 06 U23）：机主这一侧（[`Core::machine_side`]：机主的、包的账号的）的会话拿核心
//! 那一个（`Core::tools`）；别人的各一个叠在它上面的（`miyu_tool::Shelf::over`），用到时造、一直留着：拿掉机主的 `~home`（机主家目录
//! 里放的脚本、个人设置里的 MCP 服务），放进他自己那一层的脚本：照他找到的、不归包的（他家目录 `scripts/`、所有人都读得到的
//! `system/scripts/`，挂他的 `~home`），和他自己那一层包带的（挂包的编号）。和底下撞了名字的不登记。
//!
//! 核心那一个换代了（装卸包、扩展登记、机主的脚本改了），叠在上面的读的时候跟着重拼；核心重找脚本时（`follow_scripts`）各人那一层
//! 也重找；他的 `scripts/` 变了，重找他那一层。执行器、权限策略、放行出沙盒认来处、回合开头换快照都照会话手里这一个。他那一层的
//! 脚本照他的会话的沙盒跑（MB-1），放行出沙盒对他一律不算（`miyu_session::Confines::of`）。核心只记他的架子的弱引用：看着他
//! `scripts/` 的监视挂在架子上（`Shelf::over_keeping`），他的会话都卸了、没人拿着架子了，监视跟着放掉，人多了句柄不攒着（施工
//! U23 中下补）；再开会话时现造一个。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use miyu_kernel::id::AccountId;
use miyu_store::packages::Layer;
use miyu_store::scripts::{HOME_PACKAGE, Script};
use miyu_store::watch::{Watch, watch_dir};
use miyu_tool::{Catalog, Compose, Shelf, Tool, WeakShelf};

use crate::Core;
use crate::scripts::package;

/// 运行日志的来处。
const TARGET: &str = "scripts";

/// 造好的那几个：账号到他的。
#[derive(Default)]
pub(crate) struct Shelves(Mutex<BTreeMap<AccountId, Mine>>);

/// 一个人的：他的架子（只记弱引用：看着他 `scripts/` 的监视挂在架子上，他的会话都卸了、没人拿着架子了，监视跟着放掉，施工 U23
/// 中下补）、他那一层登记上的脚本。
struct Mine {
    shelf: WeakShelf,
    scripts: Arc<Vec<Script>>,
}

impl std::fmt::Debug for Shelves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let people = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        f.debug_list().entries(people.keys()).finish()
    }
}

/// 他那一层：脚本和照它们造好的工具（包的编号、工具）。
type OwnLayer = (Vec<Script>, Vec<(String, Arc<dyn Tool>)>);

impl Core {
    /// 属主是 `account` 的会话拿的工具目录架子：机主这一侧的是核心那一个；别人的是叠在上面的：还有会话拿着的照旧那一个，没有的
    /// 现造（找他那一层、看着他的 `scripts/`）。碰磁盘。
    pub(crate) fn shelf_of(self: &Arc<Self>, account: &AccountId) -> Shelf {
        if self.machine_side(account) {
            return self.tools.clone();
        }
        if let Some(shelf) = self.live(account) {
            return shelf;
        }
        let (scripts, tools) = self.own_layer(account);
        let watch = self.watch_own(account);
        let shelf = Shelf::over_keeping(
            &self.tools,
            compose(tools),
            watch.map(|watch| Box::new(Mutex::new(watch)) as Box<dyn std::any::Any + Send + Sync>),
        );
        let mut people = self.people();
        if let Some(alive) = people.get(account).and_then(|mine| mine.shelf.upgrade()) {
            return alive;
        }
        people.insert(
            account.clone(),
            Mine {
                shelf: shelf.downgrade(),
                scripts: Arc::new(scripts),
            },
        );
        shelf
    }

    /// `account` 还有会话拿着的那个架子。
    fn live(&self, account: &AccountId) -> Option<Shelf> {
        self.people().get(account)?.shelf.upgrade()
    }

    /// 只看一眼 `account` 的工具目录（手里没有核心的 `Arc` 的：显示名、预设页）：机主这一侧的是核心那一份；造过架子的照他的；还没造
    /// 的是拿掉 `~home` 的核心那一份（他那一层等他开会话才找）。
    pub(crate) fn catalog_of(&self, account: &AccountId) -> Catalog {
        if self.machine_side(account) {
            return self.tools.current();
        }
        match self.live(account) {
            Some(shelf) => shelf.current(),
            None => self.tools.current().without(HOME_PACKAGE),
        }
    }

    /// `account` 看得到的登记上的脚本（预设的功能表、预设页、工具的来处要的）：机主这一侧的是核心登记的全部；别人的是核心登记的
    /// 里不挂 `~home` 的（包带的），加他自己那一层的（有会话拿着他的架子的照那一份，没有的现找）。碰磁盘。
    pub(crate) fn scripts_for(&self, account: &AccountId) -> Vec<Script> {
        let all = self.scripts();
        if self.machine_side(account) {
            return all.to_vec();
        }
        let kept = self
            .people()
            .get(account)
            .filter(|mine| mine.shelf.upgrade().is_some())
            .map(|mine| Arc::clone(&mine.scripts));
        let own = kept.unwrap_or_else(|| Arc::new(self.own_layer(account).0));
        all.iter()
            .filter(|script| package(script) != HOME_PACKAGE)
            .chain(own.iter())
            .cloned()
            .collect()
    }

    /// 还有会话拿着架子的每个人那一层都重找一遍（核心重找脚本以后，`follow_scripts`：装卸包会让他那一层的包变）；没人拿着的
    /// 记录拿掉。碰磁盘。
    pub(crate) fn refresh_own_shelves(&self) {
        self.people()
            .retain(|_, mine| mine.shelf.upgrade().is_some());
        let people: Vec<AccountId> = self.people().keys().cloned().collect();
        for account in people {
            self.refresh_own(&account);
        }
    }

    /// 重找 `account` 那一层，换进他的架子。没有会话拿着他的架子的什么都不做。碰磁盘。
    fn refresh_own(&self, account: &AccountId) {
        if self.live(account).is_none() {
            return;
        }
        let (scripts, tools) = self.own_layer(account);
        let mut people = self.people();
        let Some(mine) = people.get_mut(account) else {
            return;
        };
        if let Some(shelf) = mine.shelf.upgrade() {
            shelf.recompose(compose(tools));
            mine.scripts = Arc::new(scripts);
        }
    }

    /// 找 `account` 那一层（他找得到的、不归包的，和他自己那一层包带的），照内置包端口造成工具；造不成的记一行 `WARN`，没设端口
    /// 的（测试里造的核心）只记下找到的。碰磁盘。
    fn own_layer(&self, account: &AccountId) -> OwnLayer {
        let packages = self.packages_of(account);
        let own: BTreeSet<&str> = packages
            .iter()
            .filter(|one| one.layer == Layer::User)
            .map(|one| one.id.as_str())
            .collect();
        let (found, _) = miyu_store::scripts::find(&packages, &self.root, account);
        let mine: Vec<Script> = found
            .into_iter()
            .filter(|script| script.package.as_deref().is_none_or(|id| own.contains(id)))
            .collect();
        let Some(builtins) = &self.builtins else {
            return (mine, Vec::new());
        };
        let mut made = Vec::new();
        let mut tools = Vec::new();
        for script in mine {
            match builtins.script(&script.head, script.file.clone(), package(&script)) {
                Ok(tool) => {
                    tools.push((package(&script).to_string(), tool));
                    made.push(script);
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, error = error.as_str(), "script not made");
                }
            }
        }
        (made, tools)
    }

    /// 看着 `account` 的 `scripts/`（同 `watch_scripts`）：没有的先建上；变了重找他那一层。建不了、看不了的记一行 `WARN`，交回
    /// 空的：照旧在核心重找脚本时跟着找。
    fn watch_own(self: &Arc<Self>, account: &AccountId) -> Option<Watch> {
        let dir = miyu_store::scripts::own_dir(&self.root, account);
        if let Err(error) = std::fs::create_dir_all(&dir) {
            tracing::warn!(target: TARGET, error = %error, "own scripts not watched");
            return None;
        }
        let (core, account) = (Arc::downgrade(self), account.clone());
        let watching = watch_dir(&dir, move |_| {
            if let Some(core) = core.upgrade() {
                core.refresh_own(&account);
            }
        });
        match watching {
            Ok(watch) => Some(watch),
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "own scripts not watched");
                None
            }
        }
    }

    /// 造好的那几个，拿着锁。
    fn people(&self) -> MutexGuard<'_, BTreeMap<AccountId, Mine>> {
        self.shelves
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// 他那一层怎么拼：底下那一份拿掉机主的 `~home`，加上他的脚本（撞了名字的不登记）。
fn compose(tools: Vec<(String, Arc<dyn Tool>)>) -> Compose {
    Arc::new(move |catalog: &Catalog| catalog.without(HOME_PACKAGE).with_scripts(tools.clone()).0)
}

#[cfg(test)]
mod tests;
