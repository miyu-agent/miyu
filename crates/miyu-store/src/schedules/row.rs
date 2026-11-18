//! 唤醒表的一行怎么写、怎么读（施工 J-1 上）：什么时候、跑在哪、谁建的存成 JSON 一格；跑在哪个会话另存一格好查。

use rusqlite::{Connection, Row, params};

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;

use super::{Last, Loop, Schedule, Target, Unreadable};
use crate::sqlite::DbError;

/// 一行的几格，读的顺序同 [`read`]。
const COLUMNS: &str = "id, created_at, by, when_json, prompt, target, loop, paused, next_at, last_at, last_result, \
     last_session";

/// 一个。
pub(super) const SELECT_ONE: &str = concat!(
    "SELECT id, created_at, by, when_json, prompt, target, loop, paused, next_at, last_at, last_result, last_session ",
    "FROM schedules WHERE id = ?1"
);

/// 全部：有下一次、没停的照时刻在前；停着的、没定下一次的在后，照编号的数。
pub(super) const SELECT_ALL: &str = concat!(
    "SELECT id, created_at, by, when_json, prompt, target, loop, paused, next_at, last_at, last_result, last_session ",
    "FROM schedules ORDER BY (paused = 0 AND next_at IS NOT NULL) DESC, CASE WHEN paused = 0 THEN next_at END, ",
    "CAST(substr(id, 2) AS INTEGER)"
);

/// 到点了的：没停、下一次不晚于 `?1`。
pub(super) const SELECT_DUE: &str = concat!(
    "SELECT id, created_at, by, when_json, prompt, target, loop, paused, next_at, last_at, last_result, last_session ",
    "FROM schedules WHERE paused = 0 AND next_at IS NOT NULL AND next_at <= ?1 ",
    "ORDER BY next_at, CAST(substr(id, 2) AS INTEGER)"
);

/// 写进一行新的。
pub(super) fn insert(db: &Connection, schedule: &Schedule) -> Result<(), DbError> {
    let cells = Cells::of(schedule)?;
    db.execute(
        &format!(
            "INSERT INTO schedules ({COLUMNS}, target_session) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, \
             ?11, ?12, ?13)"
        ),
        params![
            schedule.id,
            schedule.created.unix_millis(),
            cells.by,
            cells.when,
            schedule.prompt,
            cells.target,
            cells.looping,
            schedule.paused,
            cells.next,
            cells.last_at,
            cells.last_result,
            cells.last_session,
            cells.target_session,
        ],
    )?;
    Ok(())
}

/// 换掉编号相同的一行（编号、建的时刻、谁建的不变）；没有的交回假。
pub(super) fn update(db: &Connection, schedule: &Schedule) -> Result<bool, DbError> {
    let cells = Cells::of(schedule)?;
    let changed = db.execute(
        "UPDATE schedules SET when_json = ?2, prompt = ?3, target = ?4, loop = ?5, paused = ?6, next_at = ?7, \
         last_at = ?8, last_result = ?9, last_session = ?10, target_session = ?11 WHERE id = ?1",
        params![
            schedule.id,
            cells.when,
            schedule.prompt,
            cells.target,
            cells.looping,
            schedule.paused,
            cells.next,
            cells.last_at,
            cells.last_result,
            cells.last_session,
            cells.target_session,
        ],
    )?;
    Ok(changed > 0)
}

/// 只写上一次那几格；`stop` 的同时停掉、清掉下一次。
pub(super) fn record(db: &Connection, id: &str, last: &Last, stop: bool) -> Result<bool, DbError> {
    let changed = db.execute(
        "UPDATE schedules SET last_at = ?2, last_result = ?3, last_session = ?4, \
         paused = CASE WHEN ?5 THEN 1 ELSE paused END, next_at = CASE WHEN ?5 THEN NULL ELSE next_at END WHERE id = ?1",
        params![
            id,
            last.at.unix_millis(),
            last.result,
            last.session.as_ref().map(SessionId::as_str),
            stop,
        ],
    )?;
    Ok(changed > 0)
}

/// 读一行：读不懂的那一格说是哪一行哪一格（外层 `Result` 是 SQLite 的，里层是读不懂的）。一格的类型不对（被人改坏了）也是
/// 这一行读不懂，不是读不了库（施工 AE-4）：交在里层，[`super::Schedules::list`]、[`super::Schedules::due`] 照它跳过这一行。
pub(super) fn read(row: &Row<'_>) -> rusqlite::Result<Result<Schedule, Unreadable>> {
    let id: String = row.get(0).unwrap_or_else(|_| "?".to_string());
    Ok((|| {
        let bad = |what: &str| Unreadable {
            id: id.clone(),
            why: format!("bad {what}"),
        };
        let time = |ms: i64, what: &str| Timestamp::from_unix_millis(ms).ok_or_else(|| bad(what));
        let created: i64 = row.get(1).map_err(|_| bad("created_at"))?;
        let by: String = row.get(2).map_err(|_| bad("by"))?;
        let when: String = row.get(3).map_err(|_| bad("when"))?;
        let prompt: String = row.get(4).map_err(|_| bad("prompt"))?;
        let target: String = row.get(5).map_err(|_| bad("target"))?;
        let looping: Option<String> = row.get(6).map_err(|_| bad("loop"))?;
        let paused: bool = row.get(7).map_err(|_| bad("paused"))?;
        let next: Option<i64> = row.get(8).map_err(|_| bad("next_at"))?;
        let last_at: Option<i64> = row.get(9).map_err(|_| bad("last_at"))?;
        let last_result: Option<String> = row.get(10).map_err(|_| bad("last_result"))?;
        let last_session: Option<String> = row.get(11).map_err(|_| bad("last_session"))?;
        let last = match (last_at, last_result) {
            (Some(at), Some(result)) => Some(Last {
                at: time(at, "last_at")?,
                result,
                session: last_session
                    .map(|session| SessionId::parse(&session).map_err(|_| bad("last_session")))
                    .transpose()?,
            }),
            _ => None,
        };
        Ok(Schedule {
            created: time(created, "created_at")?,
            by: serde_json::from_str(&by).map_err(|_| bad("by"))?,
            when: serde_json::from_str(&when).map_err(|_| bad("when"))?,
            prompt,
            target: serde_json::from_str(&target).map_err(|_| bad("target"))?,
            looping: looping
                .map(|looping| {
                    serde_json::from_value::<Loop>(looping.into()).map_err(|_| bad("loop"))
                })
                .transpose()?,
            paused,
            next: next.map(|ms| time(ms, "next_at")).transpose()?,
            last,
            id: id.clone(),
        })
    })())
}

/// 一行里要换写法的几格。
struct Cells {
    by: String,
    when: String,
    target: String,
    target_session: Option<String>,
    looping: Option<&'static str>,
    next: Option<i64>,
    last_at: Option<i64>,
    last_result: Option<String>,
    last_session: Option<String>,
}

impl Cells {
    fn of(schedule: &Schedule) -> Result<Cells, DbError> {
        let json = |value: Result<String, serde_json::Error>| {
            value.map_err(|error| DbError::Bad(format!("schedule {}: {error}", schedule.id)))
        };
        Ok(Cells {
            by: json(serde_json::to_string(&schedule.by))?,
            when: json(serde_json::to_string(&schedule.when))?,
            target: json(serde_json::to_string(&schedule.target))?,
            target_session: match &schedule.target {
                Target::Session { session } => Some(session.as_str().to_string()),
                Target::New { .. } | Target::Package { .. } => None,
            },
            looping: schedule.looping.map(|looping| match looping {
                Loop::Fixed => "fixed",
                Loop::Dynamic => "dynamic",
            }),
            next: schedule.next.map(Timestamp::unix_millis),
            last_at: schedule.last.as_ref().map(|last| last.at.unix_millis()),
            last_result: schedule.last.as_ref().map(|last| last.result.clone()),
            last_session: schedule
                .last
                .as_ref()
                .and_then(|last| last.session.as_ref())
                .map(|session| session.as_str().to_string()),
        })
    }
}
