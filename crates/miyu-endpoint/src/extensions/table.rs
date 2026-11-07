//! 照清单建的表（施工 HK-2 从 `guards.rs` 抽出来，工具调用前的守卫、调用后的通知共用）：照核心手上的那份清单建，核心起来时
//! 建一次，清单换了（装卸、升级）照新的那一份重建；同一份照旧（照 `Arc` 认）。查表只拿一次锁，不读盘。

use std::sync::{Arc, Mutex, PoisonError};

use miyu_store::packages::Found;

use crate::Core;

/// 建好的表，连同照它建的那一份清单。
type Built<E> = (Arc<Vec<Found>>, Arc<Vec<E>>);

/// 一张照清单建的表：核心一份。
pub(crate) struct Table<E>(Mutex<Option<Built<E>>>);

impl<E> Default for Table<E> {
    fn default() -> Self {
        Table(Mutex::new(None))
    }
}

impl<E> Table<E> {
    /// 照核心这时的清单拿表：和上一次是同一份的照旧，换了的照 `build` 重建。
    pub(crate) fn get(&self, core: &Core, build: fn(&[Found]) -> Vec<E>) -> Arc<Vec<E>> {
        let packages = core.system_packages();
        let mut built = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((seen, table)) = built.as_ref()
            && Arc::ptr_eq(seen, &packages)
        {
            return Arc::clone(table);
        }
        let table = Arc::new(build(&packages));
        *built = Some((packages, Arc::clone(&table)));
        table
    }
}
