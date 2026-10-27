//! 定时和循环的唤醒（施工 J-1 上，设计 `45-定时和循环.md` 第三节第 1、2 条，`docs/blueprint/schedules.md`）：核心里只有一样
//! 东西，到点把一句话交给一个会话跑一轮。表在存储那一层（`miyu_store::schedules`，一个账号一份）；这里管建、停、续、删，
//! 算下一次（`next.rs`），睡到点（`run.rs`），交进会话（`fire.rs`）。
//!
//! - 人经协议建的（施工 J-1 下）、她用工具建的（施工 J-2）、包建的（施工 J-4）都走 [`Schedules::add`]。
//! - 表改了叫醒睡着的那个（`Schedules::changed`），重算最早的一次；不轮询。
//! - 错过的不补：核心起来时已经过了的跳过（`Schedules::recall`），照现在算下一次。

pub(crate) mod changes;
mod clock;
mod fire;
mod hold;
pub(crate) mod methods;
mod next;
pub(crate) mod port;
mod run;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use jiff::tz::TimeZone;
use tokio::sync::Notify;

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;
use miyu_store::root::DataRoot;
use miyu_store::schedules::{self as table, NewSchedule, Opened, Schedules as Table};
use miyu_store::sqlite::DbError;
use miyu_tool::PER_ACCOUNT;

pub use clock::{Clock, Sleep, SystemClock};
pub use miyu_store::schedules::{Creator, Last, Loop, Schedule, Target, When};
pub use next::{MIN_EVERY, parse as parse_cron};

/// 运行日志的来源。
const TARGET: &str = "miyu::endpoint";

/// 要建的一个。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// 谁建的。
    pub by: Creator,
    /// 什么时候跑。
    pub when: When,
    /// 交进会话的那句话。
    pub prompt: String,
    /// 跑在哪。
    pub target: Target,
    /// 循环的种类；定时任务没有。
    pub looping: Option<Loop>,
    /// 第一次的时刻：不写的照 `when` 从现在往后算；`/loop` 马上先跑一次的写现在（施工 J-2）。
    pub first: Option<Timestamp>,
}

/// 建、改唤醒表出的错。
#[derive(Debug)]
pub enum ScheduleError {
    /// cron 读不成（为什么）。
    Cron(String),
    /// 某个时刻一次的已经过去了，或者往后算不出下一次。
    NoNext,
    /// 隔得太短：少于 [`MIN_EVERY`] 秒。
    TooShort,
    /// 没有这个编号。
    Unknown,
    /// 交的时区不认识（施工 J-3 上）。
    Zone,
    /// 这个账号挂满了 [`PER_ACCOUNT`] 个（施工 J-3 上）。
    TooMany,
    /// 这个账号的唤醒表这一回用不了（坏了又开不了空的、版本比这一版新）。
    Unusable,
    /// 读写库出错。
    Db(DbError),
}

/// 核心的唤醒表：各账号的表用到时开、开了一直开着；一个钟、一个叫醒睡着的那个的信号。
pub struct Schedules {
    root: DataRoot,
    clock: Arc<dyn Clock>,
    tables: Mutex<BTreeMap<AccountId, Option<Arc<Table>>>>,
    wake: Arc<Notify>,
    /// 写不进下一次的先放一放（`hold.rs`）。
    holds: hold::Holds,
    /// 哪个任务变了（`changes.rs`，施工 J-1 下）。
    changes: tokio::sync::broadcast::Sender<String>,
}

impl crate::Core {
    /// 换掉唤醒表的钟：测试用手拨的，不真等。
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> crate::Core {
        self.schedules.clock = clock;
        self
    }

    /// 唤醒表。
    pub fn schedules(&self) -> &Schedules {
        &self.schedules
    }
}

impl Schedules {
    /// 数据根 `root` 上的一份，照系统的钟看时间。
    pub(crate) fn system(root: DataRoot) -> Schedules {
        Schedules {
            root,
            clock: Arc::new(SystemClock),
            tables: Mutex::new(BTreeMap::new()),
            wake: Arc::new(Notify::new()),
            holds: hold::Holds::default(),
            changes: tokio::sync::broadcast::channel(changes::CHANGES).0,
        }
    }

    /// 现在。
    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// 给 `account` 建一个：第一次照 `wanted.first`，不写的照 `when` 从现在往后算。交回建好的一行。
    ///
    /// # Errors
    ///
    /// cron 读不成、算不出第一次、隔得太短；表用不了、写不了。
    pub fn add(&self, account: &AccountId, mut wanted: Wanted) -> Result<Schedule, ScheduleError> {
        match &mut wanted.when {
            When::Cron {
                cron, zone: named, ..
            } => {
                next::parse(cron).map_err(ScheduleError::Cron)?;
                // 时区建时记下（施工 J-3 上）：交了的要认得，没交的记这台机器这一刻的。
                match named {
                    Some(name) => {
                        TimeZone::get(name).map_err(|_| ScheduleError::Zone)?;
                    }
                    None => *named = zone().iana_name().map(str::to_string),
                }
            }
            When::Every { every } if *every < MIN_EVERY => return Err(ScheduleError::TooShort),
            When::At { .. } | When::Every { .. } => {}
        }
        // 熔断性质的安全上限（施工 J-3 上）：人建、她建、包建合计；到了只拒新建。
        if self.list(account)?.len() >= PER_ACCOUNT {
            return Err(ScheduleError::TooMany);
        }
        let now = self.now();
        let first = match wanted.first {
            Some(first) => first,
            None => next::first(&wanted.when, now, &zone()).ok_or(ScheduleError::NoNext)?,
        };
        let new = NewSchedule {
            by: wanted.by,
            when: wanted.when,
            prompt: wanted.prompt,
            target: wanted.target,
            looping: wanted.looping,
            next: Some(first),
        };
        let added = self.table(account, true)?.add(new, now)?;
        tracing::info!(target: TARGET, account = %account, schedule = %added.id, "schedule added");
        self.changed();
        self.notify(account, &added.id);
        Ok(added)
    }

    /// `account` 的全部，照下一次排（停着的、没定下一次的在后）。还没有表的是空的。
    ///
    /// # Errors
    ///
    /// 表用不了、读不了。
    pub fn list(&self, account: &AccountId) -> Result<Vec<Schedule>, ScheduleError> {
        match self.table(account, false) {
            Ok(table) => Ok(table.list()?),
            Err(ScheduleError::Unknown) => Ok(Vec::new()),
            Err(error) => Err(error),
        }
    }

    /// `account` 的编号是 `id` 的那一个。
    ///
    /// # Errors
    ///
    /// 没有（[`ScheduleError::Unknown`]），表用不了、读不了。
    pub fn get(&self, account: &AccountId, id: &str) -> Result<Schedule, ScheduleError> {
        self.table(account, false)?
            .get(id)?
            .ok_or(ScheduleError::Unknown)
    }

    /// 删掉一个。
    ///
    /// # Errors
    ///
    /// 没有，表用不了、写不了。
    pub fn cancel(&self, account: &AccountId, id: &str) -> Result<(), ScheduleError> {
        if !self.table(account, false)?.remove(id)? {
            return Err(ScheduleError::Unknown);
        }
        tracing::info!(target: TARGET, account = %account, schedule = %id, "schedule cancelled");
        self.changed();
        self.notify(account, id);
        Ok(())
    }

    /// 停一个：不到点，下一次清掉。停着的再停照旧。
    ///
    /// # Errors
    ///
    /// 没有，表用不了、写不了。
    pub fn pause(&self, account: &AccountId, id: &str) -> Result<Schedule, ScheduleError> {
        self.edit(account, id, |schedule, _| {
            schedule.paused = true;
            schedule.next = None;
            Ok(())
        })
    }

    /// 续上一个：照现在算下一次（错过的不补）。没停的照旧重算。
    ///
    /// # Errors
    ///
    /// 没有，算不出下一次（某个时刻一次的已经过了），表用不了、写不了。
    pub fn resume(&self, account: &AccountId, id: &str) -> Result<Schedule, ScheduleError> {
        self.edit(account, id, |schedule, now| {
            schedule.next =
                Some(next::first(&schedule.when, now, &zone()).ok_or(ScheduleError::NoNext)?);
            schedule.paused = false;
            Ok(())
        })
    }

    /// 改下一次（她自己定节奏的循环每轮定一次，施工 J-2）：`next` 没有的是停着等；`prompt` 写了的换掉那句话。
    ///
    /// # Errors
    ///
    /// 没有，表用不了、写不了。
    pub fn set_next(
        &self,
        account: &AccountId,
        id: &str,
        next: Option<Timestamp>,
        prompt: Option<String>,
    ) -> Result<Schedule, ScheduleError> {
        self.edit(account, id, |schedule, _| {
            schedule.next = next;
            if let Some(prompt) = prompt {
                schedule.prompt = prompt;
            }
            Ok(())
        })
    }

    /// 读出编号是 `id` 的那一个，照 `change` 改了写回去，叫醒睡着的那个。
    fn edit(
        &self,
        account: &AccountId,
        id: &str,
        change: impl FnOnce(&mut Schedule, Timestamp) -> Result<(), ScheduleError>,
    ) -> Result<Schedule, ScheduleError> {
        let table = self.table(account, false)?;
        let mut schedule = table.get(id)?.ok_or(ScheduleError::Unknown)?;
        change(&mut schedule, self.now())?;
        if !table.put(&schedule)? {
            return Err(ScheduleError::Unknown);
        }
        self.changed();
        self.notify(account, id);
        Ok(schedule)
    }

    /// 表改了：叫醒睡着的那个，重算最早的一次。
    pub(crate) fn changed(&self) {
        self.wake.notify_one();
    }

    /// 账号 `account` 的表：开过的照开过的；没开过的，`create` 的没有就建，不 `create` 的没有文件就当没有
    /// （[`ScheduleError::Unknown`]）。开不了的记一行、这一回都当用不了。
    fn table(&self, account: &AccountId, create: bool) -> Result<Arc<Table>, ScheduleError> {
        let mut tables = self
            .tables
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(found) = tables.get(account) {
            return found.clone().ok_or(ScheduleError::Unusable);
        }
        if !create && !table::path(&self.root, account).exists() {
            return Err(ScheduleError::Unknown);
        }
        let (opened, how) = Table::open(&self.root, account);
        said(account, &how);
        let opened = opened.map(Arc::new);
        tables.insert(account.clone(), opened.clone());
        opened.ok_or(ScheduleError::Unusable)
    }

    /// 开着的几份表，照账号排。
    fn opened(&self) -> Vec<(AccountId, Arc<Table>)> {
        let tables = self
            .tables
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        tables
            .iter()
            .filter_map(|(account, table)| Some((account.clone(), Arc::clone(table.as_ref()?))))
            .collect()
    }

    /// 核心起来时：数据根里有唤醒表的账号都开上，已经过了的跳过（错过的不补），照现在算下一次。
    pub(crate) fn recall(&self) {
        let accounts = self.root.accounts().unwrap_or_default();
        for account in accounts
            .iter()
            .filter(|account| table::path(&self.root, account).exists())
        {
            let Ok(table) = self.table(account, false) else {
                continue;
            };
            let now = self.now();
            match table.due(now) {
                Ok(due) => {
                    for schedule in due {
                        run::skip(&table, account, schedule, now);
                    }
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, account = %account, error = %error, "schedules not read");
                }
            }
        }
    }

    /// 最早的下一次，各账号里挑；写不进下一次、放着的照「不早于」往后挪（`hold.rs`）。
    fn earliest(&self) -> Option<Timestamp> {
        self.opened()
            .into_iter()
            .flat_map(|(account, table)| {
                let listed = table.list().unwrap_or_default();
                listed
                    .into_iter()
                    .filter(|schedule| !schedule.paused)
                    .filter_map(|schedule| {
                        let next = schedule.next?;
                        Some(self.holds.later(&account, &schedule.id, next))
                    })
                    .collect::<Vec<_>>()
            })
            .min()
    }
}

/// 开一份表是什么情形，照它记日志：坏了改名留着的、比这一版新的、开不了的都是 `WARN`（施工 J-3 起另给人发一条通知）。
fn said(account: &AccountId, how: &Opened) {
    match how {
        Opened::Kept => {}
        Opened::Created => tracing::info!(target: TARGET, account = %account, "schedules created"),
        Opened::SetAside { why, .. } => {
            tracing::warn!(target: TARGET, account = %account, error = %why, "schedules set aside");
        }
        Opened::Newer(version) => {
            tracing::warn!(target: TARGET, account = %account, version, "schedules newer");
        }
        Opened::Unusable(error) => {
            tracing::warn!(target: TARGET, account = %account, error = %error, "schedules unusable");
        }
    }
}

/// 本机的时区：cron 照它算。读不出的当 UTC。
fn zone() -> TimeZone {
    TimeZone::try_system().unwrap_or(TimeZone::UTC)
}

impl From<DbError> for ScheduleError {
    fn from(error: DbError) -> ScheduleError {
        ScheduleError::Db(error)
    }
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleError::Cron(why) => write!(f, "bad cron: {why}"),
            ScheduleError::NoNext => f.write_str("no next time"),
            ScheduleError::TooShort => write!(f, "shorter than {MIN_EVERY} seconds"),
            ScheduleError::Unknown => f.write_str("unknown schedule"),
            ScheduleError::Zone => f.write_str("unknown time zone"),
            ScheduleError::TooMany => write!(f, "more than {PER_ACCOUNT} schedules"),
            ScheduleError::Unusable => f.write_str("schedules unusable"),
            ScheduleError::Db(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ScheduleError {}
