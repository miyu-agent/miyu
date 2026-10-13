//! 一个库在哪（施工 K-8 从 `knowledge.rs` 挪出来、加了包里的，`docs/blueprint/knowledge.md`「对外的样子」）：自己的在
//! `home/<账号>/kb/<编号>/`（挂进来的也是，`kb.toml` 写着挂哪），软件包带来的在包目录的 `kb/<编号>/`（照约定，同 `skills/`）。
//! 这里管看得见哪几个、索引放哪、包里的库自动想起的开关人改了记在哪，装卸以后怎么跟上。
//!
//! - 看得见：自己的全部，加装了的包里的；同编号的自己的赢，两个包同编号的照包的编号先到先得（同技能）。
//! - 索引：自己的 `home/<账号>/index/kb/<编号>.db`；包里的 `home/<账号>/index/kb/packages/<包>/<编号>.db`（自己的编号不带
//!   `/`，撞不上）。
//! - 开关：包里的库照包里 `kb.toml` 的 `auto`；人改了的记在账号自己的 `home/<账号>/kb.toml`：`[auto]` 下一行
//!   `"<包>/<编号>" = true/false`（项目主人 2026-10-13 定：包里的库只读，自动想起人能开关，卸包以后跟着没有）。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use miyu_config::Value;
use miyu_config::edit::{Change, apply};
use miyu_kernel::id::AccountId;

use super::{Knowledge, KnowledgeError, Shelf};
use crate::layers::valid;
use crate::recall::RecallIndex;

/// 一个库在哪。
pub(super) enum Place {
    /// 自己的（连挂进来的）：`home/<账号>/kb/<编号>/`。
    Own(PathBuf),
    /// 软件包 `package` 带来的：包目录里的 `kb/<编号>/`，只读。
    Package {
        /// 包的编号。
        package: String,
        /// 库的目录。
        dir: PathBuf,
    },
}

impl Place {
    /// 库的目录：`kb.toml` 在这里。
    pub(super) fn dir(&self) -> &Path {
        match self {
            Place::Own(dir) | Place::Package { dir, .. } => dir,
        }
    }

    /// 包里的是哪个包的；自己的没有。
    pub(super) fn package(&self) -> Option<&str> {
        match self {
            Place::Own(_) => None,
            Place::Package { package, .. } => Some(package),
        }
    }
}

/// 装了的软件包：编号、包目录，照编号排。
pub(super) type Packages = Mutex<Vec<(String, PathBuf)>>;

impl Knowledge {
    /// 装了的软件包（编号、包目录）带来的库也算（施工 K-8）：清单读成了的那几个，调的一方交。
    #[must_use]
    pub fn with_packages(self, mut packages: Vec<(String, PathBuf)>) -> Knowledge {
        packages.sort();
        Knowledge {
            packages: Mutex::new(packages),
            ..self
        }
    }

    /// 换上一份装了的软件包（编号、包目录），什么都不清（施工 K-8）：核心起来时照核对过的清单换，没收的包不是卸了。
    pub fn use_packages(&self, mut packages: Vec<(String, PathBuf)>) {
        packages.sort();
        *self.packages_now() = packages;
    }

    /// 装卸以后照新的一份（编号、包目录）跟上（施工 K-8）：卸掉的包，每个账号里它的库的索引、自动想起的开关一起删（项目主人
    /// 2026-10-13 定：卸包以后跟着没有）。交回删不掉的那几处为什么（调的一方记日志），照样换上新的一份。
    #[must_use]
    pub fn follow_packages(&self, mut packages: Vec<(String, PathBuf)>) -> Vec<String> {
        packages.sort();
        let gone: Vec<String> = {
            let mut now = self.packages_now();
            let gone = now
                .iter()
                .map(|(id, _)| id.clone())
                .filter(|id| !packages.iter().any(|(kept, _)| kept == id))
                .collect();
            *now = packages;
            gone
        };
        if gone.is_empty() {
            return Vec::new();
        }
        let accounts = match self.root.accounts() {
            Ok(accounts) => accounts,
            Err(error) => return vec![format!("home: {error}")],
        };
        let mut problems = Vec::new();
        for account in accounts {
            for package in &gone {
                if let Err(error) = self.forget_package(&account, package) {
                    problems.push(format!("{account} {package}: {error}"));
                }
            }
        }
        problems
    }

    /// 账号下看得见的库的编号，照编号排（施工 K-1 下起会话照它判给不给她两件工具、她查得到哪几个）：自己的，加包里的（施工
    /// K-8）。目录不在的当一个都没有。
    ///
    /// # Errors
    ///
    /// 知识库的目录读不了。
    pub fn ids(&self, account: &AccountId) -> Result<Vec<String>, KnowledgeError> {
        let mut ids = self.own_ids(account)?;
        for (id, _, _) in self.carried() {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// 账号下的知识库都在哪个目录下。
    pub(super) fn top(&self, account: &AccountId) -> PathBuf {
        self.root.account_dir(account).join("kb")
    }

    /// 库 `id` 在哪：编号合写法；自己的目录在的是自己的，不然是包里的（施工 K-8）。
    pub(super) fn place(&self, account: &AccountId, id: &str) -> Result<Place, KnowledgeError> {
        if !valid(id) {
            return Err(KnowledgeError::UnknownBase);
        }
        let own = self.top(account).join(id);
        if own.is_dir() {
            return Ok(Place::Own(own));
        }
        self.carried()
            .into_iter()
            .find(|(carried, _, _)| carried == id)
            .map(|(_, package, dir)| Place::Package { package, dir })
            .ok_or(KnowledgeError::UnknownBase)
    }

    /// 库 `id` 的目录（`kb.toml` 在这里）。
    pub(super) fn dir(&self, account: &AccountId, id: &str) -> Result<PathBuf, KnowledgeError> {
        Ok(self.place(account, id)?.dir().to_path_buf())
    }

    /// 库 `id` 开着的索引：开过的交回同一份。没有的库照自己的那一格开一份空的。
    pub(super) fn shelf(&self, account: &AccountId, id: &str) -> Arc<Shelf> {
        let key = (account.clone(), id.to_string());
        if let Some(shelf) = self.lock().get(&key) {
            return Arc::clone(shelf);
        }
        let indexes = self.root.index(account).join("kb");
        let path = match self.place(account, id) {
            Ok(Place::Package { package, .. }) => indexes
                .join("packages")
                .join(package)
                .join(format!("{id}.db")),
            _ => indexes.join(format!("{id}.db")),
        };
        let mut open = self.lock();
        let shelf = open.entry(key).or_insert_with(|| {
            let (index, _) = RecallIndex::open(&path);
            Arc::new(Shelf {
                index: Arc::new(index),
                catching_up: Mutex::new(()),
                path: path.clone(),
            })
        });
        Arc::clone(shelf)
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, super::Open> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 包里的库 `place` 自动想起的开关人改过的（账号自己的 `kb.toml`）；自己的、没改过的、读不了的没有。
    pub(super) fn switched(&self, account: &AccountId, id: &str, place: &Place) -> Option<bool> {
        let package = place.package()?;
        let text = std::fs::read_to_string(self.switches(account)).ok()?;
        let switches = miyu_recall::kb::switches(&text).ok()?;
        switches.get(&format!("{package}/{id}")).copied()
    }

    /// 包 `package` 里的库 `id` 自动想起开、关（施工 K-8）：记在账号自己的 `kb.toml`，别的照原样。
    pub(super) fn switch(
        &self,
        account: &AccountId,
        package: &str,
        id: &str,
        auto: bool,
    ) -> Result<(), KnowledgeError> {
        let file = self.real_switches(account)?;
        let text = read_or_empty(&file)?;
        let key = format!("auto.\"{package}/{id}\"");
        let text = apply(&text, Change::Set(&key, &Value::Bool(auto))).map_err(|blocked| {
            KnowledgeError::Io(format!("kb.toml: cannot write {}", blocked.0))
        })?;
        miyu_fs::replace(&file, text.as_bytes()).map_err(io)
    }

    /// 账号自己记开关的那一份。
    fn switches(&self, account: &AccountId) -> PathBuf {
        self.root.account_dir(account).join("kb.toml")
    }

    /// 同 `switches`，换成真实的位置，写之前用：数据根的路上可能有链接（macOS 的临时目录 `/var`），
    /// `miyu_fs::replace` 一层链接都不跟。账号的目录不在的建上。
    fn real_switches(&self, account: &AccountId) -> Result<PathBuf, KnowledgeError> {
        let dir = self.root.account_dir(account);
        std::fs::create_dir_all(&dir).map_err(io)?;
        Ok(std::fs::canonicalize(dir).map_err(io)?.join("kb.toml"))
    }

    /// 自己的库的编号：`home/<账号>/kb/` 下合写法的目录。
    fn own_ids(&self, account: &AccountId) -> Result<Vec<String>, KnowledgeError> {
        let entries = match std::fs::read_dir(self.top(account)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io(error)),
        };
        Ok(entries
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| valid(name))
            .collect())
    }

    /// 装了的包带来的库：编号、包的编号、库的目录；同编号的照包的编号先到先得。包目录里没有 `kb/` 的、读不了的没有。
    fn carried(&self) -> Vec<(String, String, PathBuf)> {
        let mut carried: Vec<(String, String, PathBuf)> = Vec::new();
        for (package, dir) in self.packages_now().iter() {
            let Ok(entries) = std::fs::read_dir(dir.join("kb")) else {
                continue;
            };
            let mut found: Vec<(String, PathBuf)> = entries
                .flatten()
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .map(|entry| {
                    (
                        entry.file_name().to_string_lossy().into_owned(),
                        entry.path(),
                    )
                })
                .filter(|(id, _)| valid(id))
                .collect();
            found.sort();
            for (id, path) in found {
                if !carried.iter().any(|(taken, _, _)| *taken == id) {
                    carried.push((id, package.clone(), path));
                }
            }
        }
        carried
    }

    fn packages_now(&self) -> MutexGuard<'_, Vec<(String, PathBuf)>> {
        self.packages.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 卸掉的包 `package` 在账号 `account` 里留下的：开着的索引关上、连目录删掉，开关那几行拿掉。
    ///
    /// # Errors
    ///
    /// 索引关不上、删不掉；账号的 `kb.toml` 读不了、读不懂、写不进。
    fn forget_package(&self, account: &AccountId, package: &str) -> Result<(), KnowledgeError> {
        let indexes = self
            .root
            .index(account)
            .join("kb")
            .join("packages")
            .join(package);
        let closing: Vec<Arc<Shelf>> = {
            let mut open = self.lock();
            let keys: Vec<(AccountId, String)> = open
                .iter()
                .filter(|((owner, _), shelf)| owner == account && shelf.path.starts_with(&indexes))
                .map(|(key, _)| key.clone())
                .collect();
            keys.iter().filter_map(|key| open.remove(key)).collect()
        };
        for shelf in closing {
            let _only_one = shelf
                .catching_up
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            shelf.index.discard()?;
        }
        match std::fs::remove_dir_all(&indexes) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(io(error)),
            _ => {}
        }
        let file = self.real_switches(account)?;
        let mut text = read_or_empty(&file)?;
        let switches = miyu_recall::kb::switches(&text).map_err(KnowledgeError::Io)?;
        let prefix = format!("{package}/");
        let mine: Vec<&String> = switches
            .keys()
            .filter(|key| key.starts_with(&prefix))
            .collect();
        if mine.is_empty() {
            return Ok(());
        }
        for key in mine {
            text = apply(&text, Change::Unset(&format!("auto.\"{key}\""))).map_err(|blocked| {
                KnowledgeError::Io(format!("kb.toml: cannot write {}", blocked.0))
            })?;
        }
        miyu_fs::replace(&file, text.as_bytes()).map_err(io)
    }
}

/// 读一份字，没有的是空的。
fn read_or_empty(file: &Path) -> Result<String, KnowledgeError> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(io(error)),
    }
}

fn io(error: std::io::Error) -> KnowledgeError {
    KnowledgeError::Io(error.to_string())
}
