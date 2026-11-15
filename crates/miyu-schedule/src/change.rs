//! `note_change`（施工 LF-11，设计 `70-推演人生.md` 第六节，`docs/blueprint/tools/note_change.md`）：她在人生会话里记下、改、
//! 去掉自己的一处变化。同 `end_life` 在包 [`crate::LIFE`] 里（功能 `change`），只在没结束的人生会话的工具面上；访问类别是读，
//! 同 `remember`：动的是 Miyu 自己的记忆，不是这台机器。
//!
//! - `text` 和 `drop` 得有一个、只能有一个（`replaces` 只跟 `text`）；「没传」的几种写法（空串、`null`、`undefined`）当没传，
//!   同 `remember`。`text` 超过 [`TEXT_CHARS`] 字的拒。`why` 跟 `text` 的是为什么变的，记在那一条上；跟 `drop` 的是为什么不算了。
//! - 经记忆的端口（[`miyu_tool::MemoryPort::note`]、[`miyu_tool::MemoryPort::drop_note`]）记：规矩（只碰 `change`、上限）在
//!   端口后面的记忆那一层，这件工具不判自己在哪种会话里。没有端口的回「记忆没开」。
//! - 自己的几句在 `software/life/change/`；没有这一条、已经不算了、记忆没开、写不进、参数读不懂的照记忆包现成的
//!   `software/memory/common/` 那几句说（给人看的也照记忆包的编号）。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_recall::MemoryId;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{
    Call, Done, NOTE_CHANGE, Note, NoteRefused, Progress, Refused, Running, Spec, TEXT_CHARS, Tool,
};

use crate::LIFE_SOFTWARE;
use crate::texts::Package;

/// 这个包。
const PACKAGE: Package = Package { id: LIFE_SOFTWARE };

/// 记忆包：几句借它的（资源目录 `software/memory/common/`，给人看的 `software/memory/…`）。
const MEMORY: Package = Package { id: "memory" };

/// `note_change`。
pub(crate) struct NoteChange {
    spec: Spec,
    saved: Template,
    replaced: Template,
    dropped: Template,
    full: Template,
    not_a_note: Template,
    one_of: Template,
    too_long: Template,
    bad_args: Template,
    off: Template,
    no_such: Template,
    not_current: Template,
    failed: Template,
}

/// 她给的参数：都可以不写。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    replaces: Option<String>,
    #[serde(default)]
    drop: Option<String>,
    #[serde(default)]
    why: Option<String>,
}

/// 要做的一件。
enum Asked {
    /// 记一条，改的是 `replaces`（写了的话）。
    Note {
        text: String,
        replaces: Option<String>,
        why: Option<String>,
    },
    /// 去掉一条。
    Drop { id: String, why: String },
}

impl NoteChange {
    /// 照资源目录 `resources` 里的字造。
    pub(crate) fn load(resources: &Path) -> Result<NoteChange, LoadError> {
        let change =
            |name: &str, fields: &[&str]| PACKAGE.text(resources, ("change", name), fields);
        let common = |name: &str, fields: &[&str]| MEMORY.text(resources, ("common", name), fields);
        Ok(NoteChange {
            spec: load::spec(resources, LIFE_SOFTWARE, NOTE_CHANGE, Access::Read)?,
            saved: change("saved", &["id"])?,
            replaced: change("replaced", &["id", "old"])?,
            dropped: change("dropped", &["id"])?,
            full: change("full", &["limit"])?,
            not_a_note: change("not-a-note", &["id"])?,
            one_of: change("one-of", &[])?,
            too_long: change("too-long", &["chars", "limit"])?,
            bad_args: common("bad-args", &["error"])?,
            off: common("off", &[])?,
            no_such: common("no-such", &["id"])?,
            not_current: common("not-current", &["id"])?,
            failed: common("failed", &["error"])?,
        })
    }

    /// 没有这一条：她写的编号原样说（写法不对的也是）。
    fn no_such(&self, id: &str) -> Done {
        MEMORY.error(&self.no_such, "common/no-such", &[("id", id)])
    }

    /// 端口拒了：照哪一种说一句。
    fn refused(&self, refused: NoteRefused) -> Done {
        match refused {
            // 端口只拒成「不是变化」；记忆那一种（指着变化的拒）到不了这件工具，照同一句说。
            NoteRefused::NotANote(id) | NoteRefused::Memory(Refused::Change(id)) => PACKAGE.error(
                &self.not_a_note,
                "change/not-a-note",
                &[("id", &id.to_string())],
            ),
            NoteRefused::Full(limit) => {
                PACKAGE.error(&self.full, "change/full", &[("limit", &limit.to_string())])
            }
            NoteRefused::Memory(Refused::NoSuch(id)) => self.no_such(&id.to_string()),
            NoteRefused::Memory(Refused::NotCurrent(id)) => MEMORY.error(
                &self.not_current,
                "common/not-current",
                &[("id", &id.to_string())],
            ),
            NoteRefused::Memory(Refused::Failed(error)) => {
                MEMORY.error(&self.failed, "common/failed", &[("error", &error)])
            }
        }
    }
}

/// 「没传」的几种写法当没传：同 `remember` 的 `replaces`。
fn given(value: Option<String>) -> Option<String> {
    value.filter(|value| !matches!(value.trim(), "" | "null" | "undefined"))
}

/// 她要做的那一件：`text` 和 `drop` 只能有一个，`replaces` 只跟 `text`。都没有、都有的没有。
fn asked(args: Args) -> Option<Asked> {
    match (given(args.text), given(args.drop), given(args.replaces)) {
        (Some(text), None, replaces) => Some(Asked::Note {
            text,
            replaces,
            why: given(args.why),
        }),
        (None, Some(id), None) => Some(Asked::Drop {
            id,
            why: given(args.why).unwrap_or_default(),
        }),
        _ => None,
    }
}

impl Tool for NoteChange {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => {
                    let error = error.to_string();
                    return MEMORY.error(&self.bad_args, "common/bad-args", &[("error", &error)]);
                }
            };
            let Some(asked) = asked(args) else {
                return PACKAGE.error(&self.one_of, "change/one-of", &[]);
            };
            let Some(port) = call.memory.as_ref() else {
                return MEMORY.error(&self.off, "common/off", &[]);
            };
            match asked {
                Asked::Note {
                    text,
                    replaces,
                    why,
                } => {
                    let chars = text.chars().count();
                    if chars > TEXT_CHARS {
                        let (chars, limit) = (chars.to_string(), TEXT_CHARS.to_string());
                        return PACKAGE.error(
                            &self.too_long,
                            "change/too-long",
                            &[("chars", &chars), ("limit", &limit)],
                        );
                    }
                    let replaces = match replaces.as_deref().map(MemoryId::parse) {
                        None => None,
                        Some(Some(old)) => Some(old),
                        Some(None) => return self.no_such(replaces.as_deref().unwrap_or("")),
                    };
                    let note = Note {
                        text,
                        replaces,
                        why,
                    };
                    match port.note(note).await {
                        Ok(id) => {
                            let id = id.to_string();
                            match replaces {
                                None => PACKAGE.ok(&self.saved, "change/saved", &[("id", &id)]),
                                Some(old) => PACKAGE.ok(
                                    &self.replaced,
                                    "change/replaced",
                                    &[("id", &id), ("old", &old.to_string())],
                                ),
                            }
                        }
                        Err(refused) => self.refused(refused),
                    }
                }
                Asked::Drop { id, why } => {
                    let Some(parsed) = MemoryId::parse(&id) else {
                        return self.no_such(&id);
                    };
                    match port.drop_note(parsed, why).await {
                        Ok(()) => PACKAGE.ok(&self.dropped, "change/dropped", &[("id", &id)]),
                        Err(refused) => self.refused(refused),
                    }
                }
            }
        })
    }
}
