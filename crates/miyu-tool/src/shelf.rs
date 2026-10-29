//! 目录架子（施工 O-2 中，`docs/blueprint/providers.md`「目录换代」）：核心现在的工具目录，提供者登记一次换一代。核心、会话的
//! 执行器、权限策略、回合开头换快照的那一段拿着同一个架子：执行、判权限照现在的那一份找工具，换快照的照代数看换没换。
//!
//! 叠起来的架子（施工 U23 中中）：叠在另一个架子上面（[`Shelf::over`]）。它自己那一层是「怎么从底下那一份
//! 拼出这一份」（[`Compose`]）。读的时候照底下的代数、自己那一层的代数现拼，
//! 两边都没变的用拼好的那份；代数是两边之和，两边都只增不减，拼出来的也只增不减。

use std::any::Any;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError, RwLock, Weak};

use crate::{Catalog, Tool};

/// 自己那一层：照底下那一份拼出这一份。
pub type Compose = Arc<dyn Fn(&Catalog) -> Catalog + Send + Sync>;

/// 目录架子：克隆的是同一个架子。
#[derive(Clone)]
pub struct Shelf {
    inner: Arc<Inner>,
}

/// 一个架子：自己放着一份的，或者叠在别的架子上的。
enum Inner {
    Plain(RwLock<Edition>),
    Over(Over),
}

/// 叠在 `base` 上的：自己那一层，和上一次拼好的那份（照哪两个代数拼的）；`_keep` 跟着这个架子活（施工 U23 中下补：没有会话拿着
/// 这个架子了就一起丢掉）。
struct Over {
    base: Shelf,
    own: RwLock<Own>,
    made: Mutex<Option<Made>>,
    _keep: Option<Box<dyn Any + Send + Sync>>,
}

/// 架子的弱引用（施工 U23 中下补）：不让架子多活，拿着它的都丢了就取不回来。
#[derive(Clone)]
pub struct WeakShelf(Weak<Inner>);

impl WeakShelf {
    /// 还有人拿着这个架子就交回它。
    #[must_use]
    pub fn upgrade(&self) -> Option<Shelf> {
        self.0.upgrade().map(|inner| Shelf { inner })
    }
}

/// 自己那一层：换过几次、怎么拼。
struct Own {
    generation: u64,
    compose: Compose,
}

/// 拼好的一份：照底下的第几代、自己那一层的第几代拼的。
struct Made {
    base: u64,
    own: u64,
    edition: Edition,
}

/// 架子上的一代：第几代、那一份目录。
#[derive(Clone, Debug, Default)]
pub struct Edition {
    /// 第几代：造架子时是 0，换一次加一；叠起来的是底下的和自己那一层的之和。
    pub generation: u64,
    /// 这一代的目录。
    pub catalog: Catalog,
}

impl Default for Shelf {
    fn default() -> Shelf {
        Shelf::new(Catalog::default())
    }
}

impl Shelf {
    /// 架子上放着 `catalog`，第 0 代。
    #[must_use]
    pub fn new(catalog: Catalog) -> Shelf {
        Shelf {
            inner: Arc::new(Inner::Plain(RwLock::new(Edition {
                generation: 0,
                catalog,
            }))),
        }
    }

    /// 叠在 `base` 上的架子（施工 U23 中中）：目录照 `compose` 从 `base` 现在的那一份拼。底下换了的这里跟着换。
    #[must_use]
    pub fn over(base: &Shelf, compose: Compose) -> Shelf {
        Shelf::over_keeping(base, compose, None)
    }

    /// 同 [`Shelf::over`]，`keep` 跟着这个架子活（施工 U23 中下补）：拿着这个架子的都丢了，`keep` 一起丢掉。
    #[must_use]
    pub fn over_keeping(
        base: &Shelf,
        compose: Compose,
        keep: Option<Box<dyn Any + Send + Sync>>,
    ) -> Shelf {
        Shelf {
            inner: Arc::new(Inner::Over(Over {
                base: base.clone(),
                own: RwLock::new(Own {
                    generation: 0,
                    compose,
                }),
                made: Mutex::new(None),
                _keep: keep,
            })),
        }
    }

    /// 弱引用（施工 U23 中下补）。
    #[must_use]
    pub fn downgrade(&self) -> WeakShelf {
        WeakShelf(Arc::downgrade(&self.inner))
    }

    /// 叠起来的架子换成照 `compose` 拼（施工 U23 中中），算一代；不是叠起来的什么都不做，交回假。
    pub fn recompose(&self, compose: Compose) -> bool {
        let Inner::Over(over) = &*self.inner else {
            return false;
        };
        let mut own = over.own.write().unwrap_or_else(PoisonError::into_inner);
        own.generation += 1;
        own.compose = compose;
        true
    }

    /// 现在这一代。
    #[must_use]
    pub fn edition(&self) -> Edition {
        match &*self.inner {
            Inner::Plain(edition) => edition
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            Inner::Over(over) => over.edition(),
        }
    }

    /// 现在的目录。
    #[must_use]
    pub fn current(&self) -> Catalog {
        self.edition().catalog
    }

    /// 现在的目录里叫 `name` 的那一件（[`Catalog::get`]）。
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.read(|catalog| catalog.get(name).cloned())
    }

    /// 现在的目录里叫 `name` 的那一件归哪个包（[`Catalog::package_of`]）：主人放行出沙盒的照它认来处（施工 X-36）。
    #[must_use]
    pub fn package_of(&self, name: &str) -> Option<String> {
        self.read(|catalog| catalog.package_of(name).map(str::to_string))
    }

    /// 叫 `name` 的那一件是随包卸掉的（施工 F-5 中，[`Catalog::gone`]）。
    #[must_use]
    pub fn gone(&self, name: &str) -> bool {
        self.read(|catalog| catalog.gone(name))
    }

    /// 照现在的目录换一份：`change` 交回新的就换上、算一代，交回第几代；交回错误的不动。两处同时换的一个接一个，
    /// 后换的照先换的结果改。叠起来的架子换的是最底下那一个（会话不换架子，换的只有核心）。
    ///
    /// # Errors
    ///
    /// `change` 交回的错误。
    pub fn replace<E>(
        &self,
        change: impl FnOnce(&Catalog) -> Result<Catalog, E>,
    ) -> Result<u64, E> {
        match &*self.inner {
            Inner::Plain(edition) => {
                let mut edition = edition.write().unwrap_or_else(PoisonError::into_inner);
                edition.catalog = change(&edition.catalog)?;
                edition.generation += 1;
                Ok(edition.generation)
            }
            Inner::Over(over) => over.base.replace(change),
        }
    }

    /// 照现在的目录看一眼：自己放着的不拷整份。
    fn read<T>(&self, look: impl FnOnce(&Catalog) -> T) -> T {
        match &*self.inner {
            Inner::Plain(edition) => look(
                &edition
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .catalog,
            ),
            Inner::Over(over) => look(&over.edition().catalog),
        }
    }
}

impl Over {
    /// 底下的、自己那一层的代数都没变的交回拼好的那份，变了的现拼。
    fn edition(&self) -> Edition {
        let base = self.base.edition();
        let own = self.own.read().unwrap_or_else(PoisonError::into_inner);
        let mut made = self.made.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(made) = made
            .as_ref()
            .filter(|made| made.base == base.generation && made.own == own.generation)
        {
            return made.edition.clone();
        }
        let edition = Edition {
            generation: base.generation + own.generation,
            catalog: (own.compose)(&base.catalog),
        };
        *made = Some(Made {
            base: base.generation,
            own: own.generation,
            edition: edition.clone(),
        });
        edition
    }
}

impl fmt::Debug for Shelf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.edition().fmt(f)
    }
}

#[cfg(test)]
mod tests;
