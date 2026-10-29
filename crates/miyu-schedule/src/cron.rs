//! `cron_create`、`cron_delete`、`cron_list`（施工 J-2，设计 45 第三节第 3 条，照 Claude Code 的三件）：在这个会话上建、删、列
//! 定时任务。访问类别是读：不碰文件、不问人，只读开着也能用（设计 45：她建不用问人）。
//!
//! - `cron_create`：五段 cron、到点交进来的话、是不是循环（不写的是循环）。成了说编号和下一次几点，下一次照会话的时区写。
//!   她在这个会话里挂满了的、cron 写法不对的，端口拒，照原因说。
//! - `cron_delete`：照编号删这个会话上的一个。
//! - `cron_list`：这个会话上的，一项一行：编号、什么时候、哪一种（`once`、`recurring`、`loop:fixed`、`loop:dynamic`）、下一次
//!   （人暂停了的写 `paused`，动态循环醒过、她还没定下一次的写 `stopped`）、到点交进来的话。

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use miyu_kernel::time::UtcOffset;
use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{
    CRON_CREATE, CRON_DELETE, CRON_LIST, Call, Done, IN_ALL, PER_SESSION, Progress, Running,
    ScheduleAnswer, ScheduleAsk, ScheduleRefused, Scheduled, Spec, Tool,
};

use crate::SCHEDULE;
use crate::texts::{Common, Package, clock, line};

/// 这个包。
const PACKAGE: Package = Package { id: SCHEDULE };

/// 三件：共用一份字。
pub(crate) fn tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    let texts = Arc::new(Texts::load(resources)?);
    let make = |name: &str, which: Which| -> Result<Arc<dyn Tool>, LoadError> {
        Ok(Arc::new(Cron {
            spec: load::spec(resources, SCHEDULE, name, Access::Read)?,
            which,
            texts: Arc::clone(&texts),
        }))
    };
    Ok(vec![
        make(CRON_CREATE, Which::Create)?,
        make(CRON_DELETE, Which::Delete)?,
        make(CRON_LIST, Which::List)?,
    ])
}

/// 哪一件。
#[derive(Clone, Copy)]
enum Which {
    Create,
    Delete,
    List,
}

/// 三件之一。
struct Cron {
    spec: Spec,
    which: Which,
    texts: Arc<Texts>,
}

/// `cron_create` 的参数。
#[derive(Deserialize)]
struct Create {
    cron: String,
    prompt: String,
    #[serde(default = "recurring")]
    recurring: bool,
}

/// 不写 `recurring` 的是循环（照 Claude Code）。
fn recurring() -> bool {
    true
}

/// `cron_delete` 的参数。
#[derive(Deserialize)]
struct Delete {
    id: String,
}

impl Tool for Cron {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let texts = &self.texts;
            let ask = match self.ask(&call.args) {
                Ok(ask) => ask,
                Err(done) => return *done,
            };
            let (Some(port), Some(ids)) = (call.schedules.as_ref(), call.ids.as_ref()) else {
                return texts.common.off();
            };
            let id = match &ask {
                ScheduleAsk::Delete { id } => id.clone(),
                _ => String::new(),
            };
            match port.schedule(&ids.session, ask).await {
                Ok(answer) => texts.answered(answer, call.offset),
                Err(refused) => texts.refused(refused, &id),
            }
        })
    }
}

impl Cron {
    /// 她给的参数换成要端口办的；不对的交回出错。
    fn ask(&self, args: &str) -> Result<ScheduleAsk, Box<Done>> {
        let common = &self.texts.common;
        match self.which {
            Which::Create => {
                let create: Create = serde_json::from_str(args)
                    .map_err(|error| Box::new(common.bad_args(&error)))?;
                if create.prompt.trim().is_empty() {
                    return Err(Box::new(common.bad_args(&"prompt is empty")));
                }
                Ok(ScheduleAsk::Create {
                    cron: create.cron.trim().to_string(),
                    prompt: create.prompt,
                    recurring: create.recurring,
                })
            }
            Which::Delete => {
                let delete: Delete = serde_json::from_str(args)
                    .map_err(|error| Box::new(common.bad_args(&error)))?;
                Ok(ScheduleAsk::Delete {
                    id: delete.id.trim().to_string(),
                })
            }
            Which::List => Ok(ScheduleAsk::List),
        }
    }
}

/// 读好的几句。
struct Texts {
    common: Common,
    created: miyu_kernel::template::Template,
    full: miyu_kernel::template::Template,
    all_full: miyu_kernel::template::Template,
    bad_cron: miyu_kernel::template::Template,
    not_found: miyu_kernel::template::Template,
    deleted: miyu_kernel::template::Template,
    none: miyu_kernel::template::Template,
    row: miyu_kernel::template::Template,
}

impl Texts {
    /// 照资源目录 `resources` 读，每一份拿它要的字段试换一次。
    fn load(resources: &Path) -> Result<Texts, LoadError> {
        let cron = |name: &str, fields: &[&str]| PACKAGE.text(resources, ("cron", name), fields);
        Ok(Texts {
            common: Common::load(resources, PACKAGE)?,
            created: cron("created", &["id", "next"])?,
            full: cron("full", &["limit"])?,
            all_full: cron("all-full", &["limit"])?,
            bad_cron: cron("bad-cron", &["error"])?,
            not_found: cron("not-found", &["id"])?,
            deleted: cron("deleted", &["id"])?,
            none: cron("none", &[])?,
            row: cron("row", &["id", "when", "repeat", "next", "prompt"])?,
        })
    }

    /// 端口办成了。
    fn answered(&self, answer: ScheduleAnswer, offset: UtcOffset) -> Done {
        match answer {
            ScheduleAnswer::Created(item) => {
                let next = next(&item, offset);
                PACKAGE.ok(
                    &self.created,
                    "cron/created",
                    &[("id", &item.id), ("next", &next)],
                )
            }
            ScheduleAnswer::Deleted(item) => {
                PACKAGE.ok(&self.deleted, "cron/deleted", &[("id", &item.id)])
            }
            ScheduleAnswer::Listed(items) if items.is_empty() => {
                PACKAGE.ok(&self.none, "cron/none", &[])
            }
            ScheduleAnswer::Listed(items) => {
                let rows: Vec<String> = items.iter().map(|item| self.row(item, offset)).collect();
                let count = items.len().to_string();
                // 一个的时候说单数（`miyu_store::human` 的规矩：发的一方挑 `/one`）。
                let key = match items.len() {
                    1 => "cron/listed/one",
                    _ => "cron/listed",
                };
                Done::ok(rows.join("\n")).said(PACKAGE.said(key, &[("count", &count)]))
            }
            // 循环那两样不是这三件要的：端口答错了，照用不了说。
            ScheduleAnswer::Woken(_) | ScheduleAnswer::LoopStopped => self.common.off(),
        }
    }

    /// 列表的一行。
    fn row(&self, item: &Scheduled, offset: UtcOffset) -> String {
        let repeat = match (item.kind, item.recurring) {
            (Some(kind), _) => format!("loop:{kind}"),
            (None, true) => "recurring".to_string(),
            (None, false) => "once".to_string(),
        };
        let next = next(item, offset);
        line(
            &self.row,
            &[
                ("id", &item.id),
                ("when", &item.when),
                ("repeat", &repeat),
                ("next", &next),
                ("prompt", &item.prompt),
            ],
        )
    }

    /// 端口拒了：照哪一种说一句，找不到的带上她给的编号 `id`。
    fn refused(&self, refused: ScheduleRefused, id: &str) -> Done {
        match refused {
            ScheduleRefused::Full => {
                let limit = PER_SESSION.to_string();
                PACKAGE.error(&self.full, "cron/full", &[("limit", &limit)])
            }
            ScheduleRefused::AllFull => {
                let limit = IN_ALL.to_string();
                PACKAGE.error(&self.all_full, "cron/all-full", &[("limit", &limit)])
            }
            ScheduleRefused::BadCron(error) => {
                PACKAGE.error(&self.bad_cron, "cron/bad-cron", &[("error", &error)])
            }
            ScheduleRefused::NotFound => {
                PACKAGE.error(&self.not_found, "cron/not-found", &[("id", id)])
            }
            ScheduleRefused::NoLoop | ScheduleRefused::Unavailable => self.common.off(),
        }
    }
}

/// 下一次：人暂停了的 `paused`，停着的 `stopped`，别的照会话的时区写到分钟。
fn next(item: &Scheduled, offset: UtcOffset) -> String {
    match (item.paused, item.next) {
        (true, _) => "paused".to_string(),
        (false, None) => "stopped".to_string(),
        (false, Some(at)) => clock(at, offset),
    }
}
