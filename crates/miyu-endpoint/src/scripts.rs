//! 脚本登记进工具目录（施工 X-12 中，`docs/blueprint/skills.md`「脚本」）：核心起来时、装卸包以后照这一刻装了的包和管理员
//! 家目录里的再找一遍（`miyu_store::scripts`），经内置包的端口造成工具（`Builtins::script`，只有 `miyu-core` 编进了
//! `miyu-basesystem`），整份换进工具目录（`Catalog::replacing_scripts`）：撞了别的工具名字的不登记、记一行。每个脚本算一个
//! 功能 `script-<名字>`（[`features`]），预设照它开关。
//!
//! 家目录里自己放的不归哪个包，登记时挂在 [`miyu_store::scripts::HOME_PACKAGE`] 下。核心跟着管理员家目录里的 `scripts/`
//! （[`Core::watch_scripts`]，施工 X-12 补，2026-10-13 项目主人照推荐定）：新放的、改了的、删了的当场再找一遍，下一轮的工具面
//! 跟着变，和个人设置里的 MCP 服务一样。不每轮重找：没动的时候工具面一字不变。列给头看的 `script.list` 在 `list.rs`（施工 X-27）。

use std::sync::{Arc, Mutex, PoisonError, RwLock};

use miyu_policy::features::{Feature, Features};
use miyu_policy::script::feature_id;
use miyu_store::scripts::{HOME_PACKAGE, Script};
use miyu_store::watch::{Watch, watch_dir};

use crate::Core;

pub(crate) mod list;

/// 运行日志的来处。
const TARGET: &str = "scripts";

/// 核心手里这一刻登记上的脚本；再找一遍的一次一个（起来时、装卸包、家目录变了可能同时来）。
#[derive(Default)]
pub(crate) struct Scripts {
    now: RwLock<Arc<Vec<Script>>>,
    following: Mutex<()>,
}

impl Core {
    /// 这一刻登记上的脚本，照工具名排。
    pub fn scripts(&self) -> Arc<Vec<Script>> {
        Arc::clone(
            &self
                .scripts
                .now
                .read()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// 跟着管理员家目录里的 `scripts/`（施工 X-12 补）：没有的先建上；里面点开头的以外哪个文件变了，照 `follow_scripts`
    /// 再找一遍。交回监视，丢掉就停。建不了、监视起不来的记一行 `WARN`，交回空的：照旧只在起来时、装卸包时找。
    pub fn watch_scripts(self: &Arc<Self>) -> Option<Watch> {
        let dir = miyu_store::scripts::own_dir(&self.root, &self.admin);
        if let Err(error) = std::fs::create_dir_all(&dir) {
            tracing::warn!(target: TARGET, path = %dir.display(), error = %error, "scripts not watched");
            return None;
        }
        let core = Arc::downgrade(self);
        let watching = watch_dir(&dir, move |_| {
            if let Some(core) = core.upgrade() {
                core.follow_scripts();
            }
        });
        match watching {
            Ok(watch) => {
                if let Some(why) = watch.unavailable() {
                    tracing::warn!(target: TARGET, why, "scripts watch unavailable, polling");
                }
                Some(watch)
            }
            Err(error) => {
                tracing::warn!(target: TARGET, path = %dir.display(), error = %error, "scripts not watched");
                None
            }
        }
    }

    /// 照这一刻装了的包、管理员家目录再找一遍脚本，整份换进工具目录。没设内置包端口的（测试里造的核心）只记下找到的，不登记。
    /// 碰磁盘：在起来的时候、装卸包的阻塞路径上调。
    pub(crate) fn follow_scripts(&self) {
        let _one = self
            .scripts
            .following
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (found, skipped) = miyu_store::scripts::find(&self.packages(), &self.root, &self.admin);
        for one in &skipped {
            tracing::warn!(target: TARGET, path = %one.path.display(), why = one.why.as_str(), "script skipped");
        }
        let Some(builtins) = &self.builtins else {
            *self
                .scripts
                .now
                .write()
                .unwrap_or_else(PoisonError::into_inner) = Arc::new(found);
            return;
        };
        let mut tools = Vec::new();
        let mut made = Vec::new();
        for script in found {
            match builtins.script(&script.head, script.file.clone()) {
                Ok(tool) => {
                    tools.push((package(&script).to_string(), tool));
                    made.push(script);
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, path = %script.file.display(), error = error.as_str(), "script not made");
                }
            }
        }
        let mut clashed = Vec::new();
        let replaced = self.tools.replace(|catalog| {
            let (next, skipped) = catalog.replacing_scripts(tools);
            clashed = skipped;
            Ok::<_, std::convert::Infallible>(next)
        });
        if let Err(never) = replaced {
            match never {}
        }
        for error in &clashed {
            tracing::warn!(target: TARGET, error = %error, "script not registered");
        }
        made.retain(|script| !clashed.iter().any(|error| error.tool == script.head.name));
        *self
            .scripts
            .now
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Arc::new(made);
    }
}

/// 脚本登记在哪个包下：包带的照包的编号，家目录里放的是 [`HOME_PACKAGE`]。
pub(crate) fn package(script: &Script) -> &str {
    script.package.as_deref().unwrap_or(HOME_PACKAGE)
}

/// 装了的功能，加上每个登记上的脚本一个（施工 X-12 中）：编号 `script-<名字>`，`script` 是它的工具名；个人设置里的 MCP
/// 服务每个一个，挂在 [`HOME_PACKAGE`] 下（施工 F-9 下）。包里被它盖住的同名服务的功能留着也碰不到工具：那个服务不起，
/// 目录里没有挂在那个包下的工具。
pub(crate) fn features(core: &Core) -> Features {
    let packaged = crate::packages::features(&core.packages());
    let images = packaged.image_packages().clone();
    let mut list: Vec<Feature> = packaged.iter().cloned().collect();
    list.extend(core.home_mcp.now().iter().map(|server| Feature {
        id: server.feature_id(),
        package: HOME_PACKAGE.to_string(),
        tools: Vec::new(),
        service: Some(server.name.clone()),
        script: None,
    }));
    list.extend(core.scripts().iter().map(|script| Feature {
        id: feature_id(&script.head.name),
        package: package(script).to_string(),
        tools: Vec::new(),
        service: None,
        script: Some(script.head.name.clone()),
    }));
    Features::new(list).needing_images(images)
}
