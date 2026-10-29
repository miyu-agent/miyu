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
pub(crate) mod notices;
pub(crate) mod port;
mod ran;
mod run;

use std::sync::{Arc, Mutex};

use jiff::tz::TimeZone;
use tokio::sync::Notify;

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
    /// 挂满了 [`PER_ACCOUNT`] 个（施工 J-3 上）。
    TooMany,
    /// 唤醒表这一回用不了（坏了又开不了空的、版本比这一版新）。
    Unusable,
    /// 读写库出错。
    Db(DbError),
}

/// 核心的唤醒表：用到时开、开了一直开着；一个钟、一个叫醒睡着的那个的信号。
pub struct Schedules {
    root: DataRoot,
    clock: Arc<dyn Clock>,
    /// 开过的表：没开过的是空的；开过、用不了的是 `Some(None)`。
    table: Mutex<Option<Option<Arc<Table>>>>,
    wake: Arc<Notify>,
    /// 写不进下一次的先放一放（`hold.rs`）。
    holds: hold::Holds,
    /// 哪个任务变了（`changes.rs`，施工 J-1 下）。
    changes: tokio::sync::broadcast::Sender<String>,
    /// 给人的通知（`notices.rs`，施工 J-3 下）。
    notices: notices::Notices,
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
            table: Mutex::new(None),
            wake: Arc::new(Notify::new()),
            holds: hold::Holds::default(),
            changes: tokio::sync::broadcast::channel(changes::CHANGES).0,
            notices: notices::Notices::default(),
        }
    }

    /// 给人的通知（施工 J-3 下）：订阅 `notices` 的连接照它推。
    pub(crate) fn notices(&self) -> &notices::Notices {
        &self.notices
    }

    /// 现在。
    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// 建一个：第一次照 `wanted.first`，不写的照 `when` 从现在往后算。交回建好的一行。
    ///
    /// # Errors
    ///
    /// cron 读不成、算不出第一次、隔得太短；表用不了、写不了。
    pub fn add(&self, mut wanted: Wanted) -> Result<Schedule, ScheduleError> {
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
        if self.list()?.len() >= PER_ACCOUNT {
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
        let added = self.table(true)?.add(new, now)?;
        tracing::info!(target: TARGET, schedule = %added.id, "schedule added");
        self.changed();
        self.notify(&added.id);
        Ok(added)
    }

    /// 全部，照下一次排（停着的、没定下一次的在后）。还没有表的是空的。
    ///
    /// # Errors
    ///
    /// 表用不了、读不了。
    pub fn list(&self) -> Result<Vec<Schedule>, ScheduleError> {
        match self.table(false) {
            Ok(table) => Ok(table.list()?),
            Err(ScheduleError::Unknown) => Ok(Vec::new()),
            Err(error) => Err(error),
        }
    }

    /// 编号是 `id` 的那一个。
    ///
    /// # Errors
    ///
    /// 没有（[`ScheduleError::Unknown`]），表用不了、读不了。
    pub fn get(&self, id: &str) -> Result<Schedule, ScheduleError> {
        self.table(false)?.get(id)?.ok_or(ScheduleError::Unknown)
    }

    /// 删掉一个。
    ///
    /// # Errors
    ///
    /// 没有，表用不了、写不了。
    pub fn cancel(&self, id: &str) -> Result<(), ScheduleError> {
        if !self.table(false)?.remove(id)? {
            return Err(ScheduleError::Unknown);
        }
        tracing::info!(target: TARGET, schedule = %id, "schedule cancelled");
        self.changed();
        self.notify(id);
        Ok(())
    }

    /// 停一个：不到点，下一次清掉。停着的再停照旧。
    ///
    /// # Errors
    ///
    /// 没有，表用不了、写不了。
    pub fn pause(&self, id: &str) -> Result<Schedule, ScheduleError> {
        self.edit(id, |schedule, _| {
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
    pub fn resume(&self, id: &str) -> Result<Schedule, ScheduleError> {
        self.edit(id, |schedule, now| {
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
        id: &str,
        next: Option<Timestamp>,
        prompt: Option<String>,
    ) -> Result<Schedule, ScheduleError> {
        self.edit(id, |schedule, _| {
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
        id: &str,
        change: impl FnOnce(&mut Schedule, Timestamp) -> Result<(), ScheduleError>,
    ) -> Result<Schedule, ScheduleError> {
        let table = self.table(false)?;
        let mut schedule = table.get(id)?.ok_or(ScheduleError::Unknown)?;
        change(&mut schedule, self.now())?;
        if !table.put(&schedule)? {
            return Err(ScheduleError::Unknown);
        }
        self.changed();
        self.notify(id);
        Ok(schedule)
    }

    /// 表改了：叫醒睡着的那个，重算最早的一次。
    pub(crate) fn changed(&self) {
        self.wake.notify_one();
    }

    /// 表：开过的照开过的；没开过的，`create` 的没有就建，不 `create` 的没有文件就当没有（[`ScheduleError::Unknown`]）。开不了的
    /// 记一行、这一回都当用不了。
    fn table(&self, create: bool) -> Result<Arc<Table>, ScheduleError> {
        let mut table = self
            .table
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(found) = table.as_ref() {
            return found.clone().ok_or(ScheduleError::Unusable);
        }
        if !create && !table::path(&self.root).exists() {
            return Err(ScheduleError::Unknown);
        }
        let (opened, how) = Table::open(&self.root);
        said(&how, (&self.notices, self.now()));
        let opened = opened.map(Arc::new);
        *table = Some(opened.clone());
        opened.ok_or(ScheduleError::Unusable)
    }

    /// 开着的表：没开过、用不了的没有。
    fn opened(&self) -> Option<Arc<Table>> {
        self.table
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .flatten()
    }

    /// 核心起来时：有唤醒表的开上，已经过了的跳过（错过的不补），照现在算下一次。
    pub(crate) fn recall(&self) {
        let Ok(table) = self.table(false) else {
            return;
        };
        let now = self.now();
        match table.due(now) {
            Ok(due) => {
                for schedule in due {
                    run::skip(&table, schedule, now);
                }
            }
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "schedules not read");
            }
        }
    }

    /// 最早的下一次；写不进下一次、放着的照「不早于」往后挪（`hold.rs`）。
    fn earliest(&self) -> Option<Timestamp> {
        self.opened()?
            .list()
            .unwrap_or_default()
            .into_iter()
            .filter(|schedule| !schedule.paused)
            .filter_map(|schedule| {
                let next = schedule.next?;
                Some(self.holds.later(&schedule.id, next))
            })
            .min()
    }
}

/// 开一份表是什么情形，照它记日志：坏了改名留着的、比这一版新的、开不了的都是 `WARN`。坏了换了一份空的，另给人推一条
/// `schedules.reset`（施工 J-3 下）：原来的都没了，要告诉他。
fn said(how: &Opened, (notices, now): (&notices::Notices, Timestamp)) {
    match how {
        Opened::Kept => {}
        Opened::Created => tracing::info!(target: TARGET, "schedules created"),
        Opened::SetAside { why, .. } => {
            tracing::warn!(target: TARGET, error = %why, "schedules set aside");
            notices.send(notices::RESET, now, serde_json::json!({}));
        }
        Opened::Newer(version) => {
            tracing::warn!(target: TARGET, version, "schedules newer");
        }
        Opened::Unusable(error) => {
            tracing::warn!(target: TARGET, error = %error, "schedules unusable");
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
