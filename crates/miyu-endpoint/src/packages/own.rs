//! 装着的包（施工 F-5 上、U23 末上）：[`Core::system_packages`] 是出厂的、装上的全部，进程、包账号、扩展、装卸照它；会话的工具面、
//! 技能、预设照 [`Core::packages_of`]：去掉关了的（`off.rs`）。去掉以后的那一份用到时造一份留着，连同造它时的全部和关了的：
//! 哪样换了当场重造；装卸以后另外清掉（[`Visible::forget`]）。什么都没关的就是全部本身。

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_store::packages::Found;

use crate::Core;

/// 造好的那一份。
#[derive(Debug, Default)]
pub(crate) struct Visible(Mutex<Option<Made>>);

/// 造好的一份。
#[derive(Debug)]
struct Made {
    /// 造它时的全部：和这时的不是同一份的作废。
    all: Arc<Vec<Found>>,
    /// 造它时关了的包：和这时的不一样的作废。
    off: BTreeSet<String>,
    /// 去掉关了的。
    found: Arc<Vec<Found>>,
}

impl Visible {
    /// 清掉（装着的包变了）。
    pub(crate) fn forget(&self) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

impl Core {
    /// 装着的软件包（施工 F-5 上：装卸以后当场换）：出厂的、装上的全部。机器上的事（进程、包账号、扩展、MCP 服务、装卸）照它；
    /// 会话用的照 [`Core::packages_of`]。
    pub fn system_packages(&self) -> Arc<Vec<Found>> {
        Arc::clone(&self.packages.read().unwrap_or_else(PoisonError::into_inner))
    }

    /// `account` 的会话用得到的软件包：全部去掉关了的（`Core::packages_off`）。包的账号（平台桥、替主人干活的扩展）照全部。
    pub fn packages_of(&self) -> Arc<Vec<Found>> {
        let all = self.system_packages();
        let off = self.packages_off();
        if off.is_empty() {
            return all;
        }
        let mut made = self
            .visible_packages
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(made) = made
            .as_ref()
            .filter(|made| Arc::ptr_eq(&made.all, &all) && made.off == off)
        {
            return Arc::clone(&made.found);
        }
        let found: Arc<Vec<Found>> = Arc::new(
            all.iter()
                .filter(|one| !off.contains(&one.id))
                .cloned()
                .collect(),
        );
        *made = Some(Made {
            all,
            off,
            found: Arc::clone(&found),
        });
        found
    }
}
