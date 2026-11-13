//! 她现在在干嘛（施工 LF-7，设计 `70-推演人生.md` 第三节第 8 条，`docs/blueprint/life.md`「现在在干嘛」）：人生会话里她调
//! `schedule_wakeup` 写的 `reason`，记在定时表人生那一行上（`miyu_store::schedules` 的 `doing` 表），同一个人格别的会话每一轮
//! 开头经 `HooksPort::doing` 查内存里的一份（[`Lives`]），不读盘。
//!
//! - 记：落在人生那一行上的才记（[`Schedules::set_doing`]），换行换成空格、去掉两头空白、最多 [`MOST`] 个字；她没写、写了空白
//!   的清掉。她自己定节奏的循环不记。照默认间隔醒的那一次（她没调工具）不动它：到点写下一次碰不到这张表。
//! - 内存里那一份只在 [`Schedules::notify`] 这一个口跟着表变：建、停、续、删、改下一次、到点、记这一句都报一声，照编号重读那
//!   一行。行没了（结束、表坏了换成空的）就拿掉；停着的不交。核心起来时照表铺回（[`boot`]）。
//! - 人生会话是哪个人格的：核心起来时在后台只查表里人生那几行指着的会话（[`boot`]，不扫全部会话），`life.start` 开出来时记
//!   一次。人生会话中途换了人格的，下一次核心起来才认得出（蓝图记的已知）。

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, PoisonError, RwLock};

use miyu_kernel::id::SessionId;
use miyu_kernel::template::Template;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_store::index::Life;
use miyu_store::resources::ResourceRoot;
use miyu_store::schedules::Doing;
use miyu_tool::DoingLine;

use super::{Loop, ScheduleError, Schedules, TARGET, Target};
use crate::Core;

/// 她写的那一句最多留几个字（照字符数）：多的截掉。
pub const MOST: usize = 200;

/// 内存里那一份：人生那一行的编号对它的会话、在不在跑、她写的那一句；人生会话对它的人格。
#[derive(Default)]
pub(crate) struct Lives {
    now: RwLock<Now>,
    /// 那一行的外壳：第一次用到时读，读不成的记一行、以后都当没有。
    shell: OnceLock<Option<Template>>,
}

#[derive(Default)]
struct Now {
    rows: BTreeMap<String, Lived>,
    names: BTreeMap<SessionId, String>,
}

/// 人生那一行这一刻的样子。
struct Lived {
    session: SessionId,
    running: bool,
    doing: Option<Doing>,
}

impl Lives {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, Now> {
        self.now.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Now> {
        self.now.write().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Schedules {
    /// 人生那一行 `id` 上记下她这时在干嘛：`reason` 是她写的（没写的清掉），`until` 是她刚定的下次醒的时刻。换行换成空格、去掉
    /// 两头空白、最多 [`MOST`] 个字；清理完是空的当没写。写完报一声（`Schedules::notify`），内存里那一份跟着变。
    ///
    /// # Errors
    ///
    /// 表用不了、写不了。
    pub fn set_doing(
        &self,
        id: &str,
        reason: Option<&str>,
        until: Timestamp,
    ) -> Result<(), ScheduleError> {
        let doing = reason
            .map(cleaned)
            .filter(|text| !text.is_empty())
            .map(|text| Doing {
                text,
                at: self.now(),
                until,
            });
        self.table(false)?.set_doing(id, doing.as_ref())?;
        self.notify(id);
        Ok(())
    }

    /// 任务 `id` 上记着的她在干嘛；没写过的、清掉了的没有。
    ///
    /// # Errors
    ///
    /// 表用不了、读不了。
    pub fn doing(&self, id: &str) -> Result<Option<Doing>, ScheduleError> {
        Ok(self.table(false)?.doing(id)?)
    }

    /// 人格 `persona` 的人生里她这时在干嘛，问的是会话 `asking`：只查内存。`asking` 就是人生会话的、人生停着的、那一行不在的、
    /// 她还没写过的、还不知道人生会话是哪个人格的（核心刚起来、还没扫完）没有。
    pub fn doing_of(&self, persona: &str, asking: &SessionId) -> Option<Doing> {
        let now = self.lives.read();
        now.rows
            .values()
            .find(|lived| now.names.get(&lived.session).map(String::as_str) == Some(persona))
            .filter(|lived| lived.running && lived.session != *asking)
            .and_then(|lived| lived.doing.clone())
    }

    /// 记下人生会话 `session` 是人格 `persona` 的（`life.start` 开出来时）。
    pub(crate) fn named(&self, session: &SessionId, persona: &str) {
        self.lives
            .write()
            .names
            .insert(session.clone(), persona.to_string());
    }

    /// 任务 `id` 变了：照表重读那一行，人生的记下，不是人生的、没了的拿掉。读不了的记一行，内存里的不动。
    pub(super) fn relive(&self, id: &str) {
        let Some(table) = self.opened() else {
            return;
        };
        let read = table.get(id).and_then(|row| {
            let row = row.filter(|row| row.looping == Some(Loop::Life));
            let doing = match &row {
                Some(_) => table.doing(id)?,
                None => None,
            };
            Ok(row.map(|row| (row, doing)))
        });
        let lived = match read {
            Ok(Some((row, doing))) => match row.target {
                Target::Session { session } => Some(Lived {
                    session,
                    running: !row.paused,
                    doing,
                }),
                Target::New { .. } | Target::Package { .. } => None,
            },
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(target: TARGET, schedule = %id, error = %error, "life doing not read");
                return;
            }
        };
        let mut now = self.lives.write();
        match lived {
            Some(lived) => now.rows.insert(id.to_string(), lived),
            None => now.rows.remove(id),
        };
    }
}

/// 核心起来时铺回（`Core::start_schedules` 里，表已经开过、错过的跳过以后）：表里人生的几行照读；那几行指着的人生会话是哪个
/// 人格的，在后台一个个照会话列表的读法查（[`crate::list::one`]：索引的那一行补到日志末尾）。只读有人生的那几个会话，表里
/// 没有人生的什么都不读：核心起来时不为它扫全部会话。
pub(super) fn boot(core: &Arc<Core>) {
    let schedules = &core.schedules;
    let Some(table) = schedules.opened() else {
        return;
    };
    let rows = match table.list() {
        Ok(rows) => rows,
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "life doing not read");
            return;
        }
    };
    let mut lived = Vec::new();
    for row in rows.iter().filter(|row| row.looping == Some(Loop::Life)) {
        schedules.relive(&row.id);
        if let Target::Session { session } = &row.target {
            lived.push(session.clone());
        }
    }
    if lived.is_empty() {
        return;
    }
    let core = Arc::downgrade(core);
    tokio::task::spawn_blocking(move || {
        let Some(core) = core.upgrade() else {
            return;
        };
        let index = core.index();
        for session in lived {
            let listed = crate::list::one(&core.root, &index, &session, false)
                .filter(|one| one.life == Life::Started);
            match listed.and_then(|one| one.persona) {
                Some(persona) => {
                    core.schedules
                        .lives
                        .write()
                        .names
                        .entry(session)
                        .or_insert(persona);
                }
                None => {
                    tracing::warn!(target: TARGET, session = %session.short(), "life session not listed");
                }
            }
        }
    });
}

/// `life.status` 的回应（`life.pause`、`life.resume` 同）多带 `doing: {"text", "at"}`：人生会话 `session` 那一行在、她写过的才带，
/// 停着的也带（头照它显示）。读不了的不带。
pub(crate) fn shown(core: &Core, session: &SessionId, reply: &mut serde_json::Value) {
    let rows = core.schedules.list().unwrap_or_default();
    let row = rows.iter().find(|one| {
        one.looping == Some(Loop::Life)
            && matches!(&one.target, Target::Session { session: on } if on == session)
    });
    if let Some(doing) = row.and_then(|row| core.schedules.doing(&row.id).ok().flatten()) {
        reply["doing"] = serde_json::json!({"text": doing.text, "at": doing.at});
    }
}

/// 会话 `asking`（人格 `persona`、时区 `offset`）这一轮开头该看到的那一行：见 [`Schedules::doing_of`]。外壳读不成的没有。
pub(crate) fn line(
    core: &Core,
    persona: &str,
    asking: &SessionId,
    offset: UtcOffset,
) -> Option<DoingLine> {
    let doing = core.schedules.doing_of(persona, asking)?;
    let shell = core
        .schedules
        .lives
        .shell
        .get_or_init(|| shell(&core.resources))
        .as_ref()?;
    let since = doing.at.local_minute(offset);
    let until = doing.until.local_minute(offset);
    let fields = BTreeMap::from([
        ("since", since.as_str()),
        ("until", until.as_str()),
        ("text", doing.text.as_str()),
    ]);
    let text = shell.render(&fields).ok()?;
    Some(DoingLine {
        text,
        key: doing.at.unix_millis().to_string(),
    })
}

/// 读外壳、试换一次：读不成、写法不对、要了别的字段的记一行，当没有。
fn shell(resources: &ResourceRoot) -> Option<Template> {
    let parsed = resources
        .life_doing()
        .map_err(|error| error.to_string())
        .and_then(|text| Template::parse(&text).map_err(|error| error.to_string()))
        .and_then(|shell| {
            let fields = BTreeMap::from([("since", ""), ("until", ""), ("text", "")]);
            let extra = shell
                .fields()
                .into_iter()
                .any(|field| !fields.contains_key(field));
            match (extra, shell.render(&fields)) {
                (false, Ok(_)) => Ok(shell),
                (true, _) => Err("unknown field".to_string()),
                (false, Err(error)) => Err(error.to_string()),
            }
        });
    match parsed {
        Ok(shell) => Some(shell),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "life doing shell not read");
            None
        }
    }
}

/// 她写的那一句清理好：换行（`\r\n`、`\n`、`\r`、U+2028、U+2029）换成空格，去掉两头空白，最多 [`MOST`] 个字。
fn cleaned(reason: &str) -> String {
    let flat = reason
        .replace("\r\n", " ")
        .replace(['\n', '\r', '\u{2028}', '\u{2029}'], " ");
    flat.trim()
        .chars()
        .take(MOST)
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[cfg(test)]
mod tests;
