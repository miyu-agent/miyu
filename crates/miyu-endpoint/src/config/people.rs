//! 每人一份配置（施工 Y-2 下下中上，设计 06 U21；2026-10-24 项目主人定「成员只继承系统层」）：机主的那一份照旧是核心手里的
//! [`Core::config`]、[`Core::hub`]；别人（成员）的各一份，用到时照机主那一份造（[`Config::of_member`]），一直留着：系统配置、
//! 共用的密钥文件跟机主那一份，个人设置、信任的记录是他自己的，`{ env = … }` 不认（核心的环境是机主的）。
//!
//! 系统配置、共用的密钥文件换了（手改、`config.set` 写系统层、`secret.set`），[`Core::spread`] 把已经开着的别人那几份一起
//! 换上，推给订阅着他们配置的连接。别人的个人设置只经核心写（开关、加载方式、信任），不另外监视：成员碰不到那台电脑。
//!
//! 锁的先后：拿着机主那一份的锁可以再拿表、再拿别人那一份（[`Core::spread`]）；[`Core::mine`] 造别人那一份时先拿机主的、
//! 放下，再拿表，不会反过来。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use miyu_config::Layer;
use miyu_kernel::id::AccountId;
use miyu_session::Configs;
use miyu_store::root::DataRoot;

use super::hub::{Hub, Pushing};
use super::{Config, Environment, File, Places, trust};

use crate::Core;

/// 别人的那几份：账号到他的配置。
#[derive(Debug, Default)]
pub(crate) struct People(Mutex<BTreeMap<AccountId, Arc<Own>>>);

/// 一个人的配置：手里的一份，换了交给谁。
#[derive(Debug)]
pub(crate) struct Own {
    config: Mutex<Config>,
    hub: Hub,
}

/// 某个账号的配置：机主的是核心手里那一份，别人的是他自己的。
pub(crate) enum Mine<'a> {
    /// 机主。
    Owner(&'a Core),
    /// 别人。
    Member(Arc<Own>),
}

impl Mine<'_> {
    /// 配置服务：同 [`Core::config`]，拿着的时候不许 `.await`。
    pub(crate) fn config(&self) -> MutexGuard<'_, Config> {
        match self {
            Mine::Owner(core) => core.config(),
            Mine::Member(own) => own.config.lock().unwrap_or_else(PoisonError::into_inner),
        }
    }

    /// 换了交给谁：会话、订阅着他的配置的连接。
    pub(crate) fn hub(&self) -> &Hub {
        match self {
            Mine::Owner(core) => &core.hub,
            Mine::Member(own) => &own.hub,
        }
    }
}

impl Core {
    /// `account` 在机主这一侧（施工 U23 中上，从 [`Core::mine`] 抽出来）：机主本人，或者包的账号（平台桥、替机主干活的扩展，
    /// 施工 Y-5 中：场所会话归它们，是机主配的）。用机主的配置、看得到机主私人的脚本和 MCP 服务。
    pub(crate) fn machine_side(&self, account: &AccountId) -> bool {
        *account == self.admin || self.package_accounts().contains(account)
    }

    /// `account` 的配置：机主的是核心手里那一份，别人的用到时造一份留着。系统账号、包的账号（平台桥、替机主干活的扩展，
    /// 施工 Y-5 中）照机主那一份：场所会话用的是机主配的模型，同它们的记忆归机主。
    pub(crate) fn mine(&self, account: &AccountId) -> Mine<'_> {
        if self.machine_side(account) {
            return Mine::Owner(self);
        }
        if let Some(own) = lock(&self.people.0).get(account) {
            return Mine::Member(Arc::clone(own));
        }
        let config = self.config().of_member(&self.root, account);
        let mut people = lock(&self.people.0);
        let own = people.entry(account.clone()).or_insert_with(|| {
            Arc::new(Own {
                hub: Hub::new(&config),
                config: Mutex::new(config),
            })
        });
        Mine::Member(Arc::clone(own))
    }

    /// `account` 的会话取配置的那一头：造会话、载入时交给它。
    pub(crate) fn configs_of(&self, account: &AccountId) -> Configs {
        self.mine(account).hub().configs()
    }

    /// 机主那一份的系统配置、共用的密钥文件换了（`owner` 是换好的那一份）：已经开着的别人那几份换上同样的，重算最终值；
    /// `pushing` 有的（系统配置变了的项）推给订阅着他们配置的连接。
    pub(crate) fn spread(&self, owner: &Config, pushing: Option<&Pushing>) {
        let people: Vec<Arc<Own>> = lock(&self.people.0).values().cloned().collect();
        for own in people {
            let mut config = own.config.lock().unwrap_or_else(PoisonError::into_inner);
            config.adopt(owner);
            own.hub.publish(&config, pushing.cloned());
        }
    }
}

/// 配置服务换上了 `config`：交给它主人的会话、订阅着的连接（同 [`Hub::publish`]）。是机主那一份的，再把系统配置、共用的
/// 密钥文件交给已经开着的别人那几份（推的只有系统配置变了的那一次）。拿着 `config` 的锁调。
pub(crate) fn publish(core: &Core, config: &Config, pushing: Option<Pushing>) {
    if config.places.account == core.admin {
        let system = pushing
            .as_ref()
            .filter(|pushing| pushing.layer == Layer::System)
            .cloned();
        core.hub.publish(config, pushing);
        core.spread(config, system.as_ref());
    } else {
        core.mine(&config.places.account)
            .hub()
            .publish(config, pushing);
    }
}

impl Config {
    /// 照机主这一份造 `account` 的：系统配置、共用的密钥文件、配置项、模型资料同这一份，个人设置、信任的记录读他自己的，
    /// 不认 `{ env = … }`。
    pub(crate) fn of_member(&self, root: &DataRoot, account: &AccountId) -> Config {
        let places = Places::of(root, account);
        let path = root.account_dir(account).join(super::PERSONAL);
        let shown = format!("home/{}/{}", account.as_str(), super::PERSONAL);
        let personal = File::read(&self.items, Layer::Personal, path, shown);
        let trust = trust::read(&places.trust).unwrap_or_default();
        let mut config = Config::assemble(
            root,
            account,
            self.home.as_deref(),
            self.items.clone(),
            BTreeMap::new(),
            [self.system.clone(), personal],
            trust,
        );
        config.environment = Environment::of(&[]);
        config.secrets = self.secrets.clone();
        let (own, shown) = super::own_secrets_at(root, account);
        config.own_secrets = super::SecretsFile::read(&own, &shown);
        config.models = self.models.clone();
        config.resolved = config.merged(None);
        config
    }

    /// 换上机主那一份 `owner` 的系统配置、共用的密钥文件，重算最终值。
    fn adopt(&mut self, owner: &Config) {
        self.system = owner.system.clone();
        self.secrets = owner.secrets.clone();
        self.resolved = self.merged(None);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
