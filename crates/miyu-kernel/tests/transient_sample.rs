//! 瞬时事件的样本（`docs/designs/samples/transient/`，`03-事件模型.md` 第五节「瞬时事件的外壳」）：
//! 除了 `model.delta` 的几种（`model.delta` 的在 `delta_sample.rs`）。
//!
//! - 45 号回复里那次 `read` 执行中的一段输出（`tool.progress`）；
//! - 44 号请求出了限速的错，等 1 秒再来的状态（`status`，施工 3-5 下）；117 号请求的限速带着 HTTP 状态码
//!   （施工 3-5 三补）；另一个会话里池的一个成员限速、换到下一个当场再来的状态（带 `failover`），和换过去成了以后推的
//!   `model.changed`（施工 8-9）；人换了模型、下一轮开始时池没了退回默认，推的 `model.changed`（`why` 是 `turn`，施工 8-10）；
//! - 54 号压缩写摘要时的两段进度（`compaction.progress`，施工 6-2 上），和压好了的那一条（`compaction.done`，
//!   施工 6-3 下）；另一个会话过了起压线、在后台提前压的那一次发出去了（`compaction.started`，施工 6-11 再补），到线时
//!   换上了它（带 `prepared`，施工 6-11 上）；
//! - 65 号回合里换上了一份待办以后推的 `todos.changed`（施工 D-3）。
//! - 65 号回合里待办的包换上了一份状态、后来清掉推的 `package.state_changed`（施工 TW-1）。
//!
//! 瞬时事件内核只推不读，所以样本在代码里照着造，不从文件读回来。

use std::fs;
use std::path::PathBuf;

use miyu_kernel::event::{
    CallError, ChangeWhy, CompactTrigger, CompactionDone, CompactionProgress, CompactionStarted,
    EffortInUse, EffortSource, ErrorClass, ModelChanged, NestedStarted, PackageStateChanged,
    RetitleFailed, Retry, Said, State, Status, Todo, TodoStatus, TodosChanged, ToolProgress,
    Transient, TransientBody, Usage,
};
use miyu_kernel::id::{CallId, CommandId, ModelName, NestedId, ProviderId, Seq, TurnId};
use miyu_kernel::origin::{By, Person, Tool};
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::ContextLimits;
use miyu_kernel::time::Timestamp;

/// 样本目录：这个 crate 的目录往上两级是仓库根。
fn samples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples")
}

/// 一份样本的每一行，去掉行尾的换行。
pub(crate) fn lines(path: &str) -> Vec<String> {
    let path = samples().join(path);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
    text.lines().map(str::to_string).collect()
}

#[test]
fn the_tool_progress_sample_is_written_exactly() {
    let call_id = CallId::parse("call_45_1").expect("样本里的调用编号合写法");
    let progress = Transient {
        at: Timestamp::parse("2026-09-25T07:04:07.901Z").expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(42).expect("42 是合法的序号"))),
        by: By::Tool(Tool { call_id }),
        cause: Some(CommandId::parse("cmd-7f3a").expect("命令编号合写法")),
        body: TransientBody::ToolProgress(ToolProgress {
            call_id,
            text: "lib.rs\n".to_string(),
        }),
    };
    assert_eq!(lines("transient/tool.progress.jsonl"), [progress.to_line()]);
}

/// 第一行是连不上、可以重试的，没有 HTTP 状态码，不写那一格；第二行是 118 号 `model.called` 的限速，带着 429
/// （施工 3-5 三补）；第三行是另一个会话里池的一个成员限速，端口换到下一个、当场再来（等 0 毫秒，带 `failover`，施工 8-9）。
#[test]
fn the_status_sample_is_written_exactly() {
    let status = |at: &str,
                  (turn, cause, seen): (u64, &str, u64),
                  (class, message): (ErrorClass, &str),
                  status: Option<u16>,
                  (wait_ms, failover): (u64, bool)| {
        Transient {
            at: Timestamp::parse(at).expect("样本的时刻合写法"),
            turn: Some(TurnId::new(Seq::new(turn).expect("合法的序号"))),
            by: By::Kernel,
            cause: Some(CommandId::parse(cause).expect("命令编号合写法")),
            body: TransientBody::Status(Status {
                seen: Seq::new(seen).expect("合法的序号"),
                retry: Retry {
                    attempt: 1,
                    limit: 5,
                    wait_ms,
                    class,
                    message: message.to_string(),
                    status,
                    failover,
                },
            }),
        }
        .to_line()
    };
    assert_eq!(
        lines("transient/status.jsonl"),
        [
            status(
                "2026-09-25T07:04:07.200Z",
                (42, "cmd-7f3a", 44),
                (ErrorClass::Retryable, "connection reset by peer"),
                None,
                (1000, false)
            ),
            status(
                "2026-09-25T07:58:12.400Z",
                (117, "cmd-d4e7", 117),
                (ErrorClass::RateLimited, "HTTP 429: Rate limit reached"),
                Some(429),
                (1000, false)
            ),
            status(
                "2026-09-25T08:20:41.300Z",
                (131, "cmd-a8c4", 132),
                (ErrorClass::RateLimited, "HTTP 429: Rate limit reached"),
                Some(429),
                (0, true)
            ),
        ]
    );
}

/// 上面第三行那一次换过去的成员成了：钉着的换成它，限额照它的，推一条 `model.changed`（施工 8-9）。第二条是样本会话 143 号
/// 回合开始时池 `free` 没了、退回 `models.chat` 的那一次（施工 8-10，和 144 号 `session.policy_changed` 对得上），带着
/// `deepseek/deepseek-v4` 配置的默认思考强度，从哪来是配置的哪一层（施工 8-18；8-18（补）起不再认会话那一层）。会话 actor
/// 造，内核不推。
#[test]
fn the_model_changed_sample_is_written_exactly() {
    let turned = Transient {
        at: Timestamp::parse("2026-09-25T21:00:00.000Z").expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(143).expect("合法的序号"))),
        by: By::Kernel,
        cause: Some(CommandId::parse("cmd-2e30").expect("命令编号合写法")),
        body: TransientBody::ModelChanged(Box::new(ModelChanged {
            reference: Some("deepseek/deepseek-v4".to_string()),
            endpoint: Some(ProviderId::parse("deepseek").expect("合编号的写法")),
            model: Some(ModelName::parse("deepseek-v4").expect("合模型名的写法")),
            effort: Some(EffortInUse {
                level: "high".to_string(),
                from: EffortSource::Session,
            }),
            limits: ContextLimits {
                window: Some(1_000_000),
                compaction_line: Some(850_000),
                context: None,
            },
            why: ChangeWhy::Turn,
        })),
    };
    let changed = Transient {
        at: Timestamp::parse("2026-09-25T08:20:44.900Z").expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(131).expect("合法的序号"))),
        by: By::Kernel,
        cause: Some(CommandId::parse("cmd-a8c4").expect("命令编号合写法")),
        body: TransientBody::ModelChanged(Box::new(ModelChanged {
            reference: Some("@duo".to_string()),
            endpoint: Some(ProviderId::parse("bigmodel").expect("合编号的写法")),
            model: Some(ModelName::parse("glm-5.3-flash").expect("合模型名的写法")),
            effort: None,
            limits: ContextLimits {
                window: Some(200_000),
                compaction_line: Some(170_000),
                context: None,
            },
            why: ChangeWhy::Failover,
        })),
    };
    assert_eq!(
        lines("transient/model.changed.jsonl"),
        [changed.to_line(), turned.to_line()]
    );
}

#[test]
fn the_compaction_progress_sample_is_written_exactly() {
    let progress = |at: &str, written: u64| {
        Transient {
            at: Timestamp::parse(at).expect("样本的时刻合写法"),
            turn: None,
            by: By::Kernel,
            cause: Some(CommandId::parse("cmd-b5e2").expect("命令编号合写法")),
            body: TransientBody::CompactionProgress(CompactionProgress {
                seen: Seq::new(53).expect("53 是合法的序号"),
                trigger: CompactTrigger::Auto,
                written,
                expected: 20000,
            }),
        }
        .to_line()
    };
    assert_eq!(
        lines("transient/compaction.progress.jsonl"),
        [
            progress("2026-09-25T07:29:58.400Z", 412),
            progress("2026-09-25T07:29:59.100Z", 957),
        ]
    );
}

#[test]
fn the_compaction_started_sample_is_written_exactly() {
    let started = Transient {
        at: Timestamp::parse("2026-10-08T03:12:21.500Z").expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(84).expect("84 是合法的序号"))),
        by: By::Kernel,
        cause: Some(CommandId::parse("cmd-c7a1").expect("命令编号合写法")),
        body: TransientBody::CompactionStarted(CompactionStarted {
            seen: Seq::new(88).expect("88 是合法的序号"),
            prepared: true,
        }),
    };
    assert_eq!(
        lines("transient/compaction.started.jsonl"),
        [started.to_line()]
    );
}

#[test]
fn the_compaction_done_sample_is_written_exactly() {
    let done = Transient {
        at: Timestamp::parse("2026-09-25T07:30:00.000Z").expect("样本的时刻合写法"),
        turn: None,
        by: By::Kernel,
        cause: Some(CommandId::parse("cmd-b5e2").expect("命令编号合写法")),
        body: TransientBody::CompactionDone(CompactionDone {
            seen: Seq::new(53).expect("53 是合法的序号"),
            trigger: CompactTrigger::Auto,
            before: 812_345,
            after: 31_020,
            usage: Some(Usage {
                uncached: 2345,
                cache_read: 810_112,
                cache_write: 0,
                output: 2412,
                reasoning: None,
            }),
            duration_ms: Some(41_250),
            prepared: false,
            truncated: None,
        }),
    };
    let prepared = Transient {
        at: Timestamp::parse("2026-10-08T03:12:40.000Z").expect("样本的时刻合写法"),
        turn: None,
        by: By::Kernel,
        cause: Some(CommandId::parse("cmd-c7a1").expect("命令编号合写法")),
        body: TransientBody::CompactionDone(CompactionDone {
            seen: Seq::new(88).expect("88 是合法的序号"),
            trigger: CompactTrigger::Auto,
            before: 152_430,
            after: 38_210,
            usage: Some(Usage {
                uncached: 1840,
                cache_read: 133_632,
                cache_write: 0,
                output: 3108,
                reasoning: None,
            }),
            duration_ms: Some(18_620),
            prepared: true,
            truncated: None,
        }),
    };
    // 摘要请求太长、截掉最老的两组才压成的（施工 UP-2）。
    let truncated = Transient {
        at: Timestamp::parse("2026-11-02T09:05:12.000Z").expect("样本的时刻合写法"),
        turn: None,
        by: By::Kernel,
        cause: Some(CommandId::parse("cmd-d3f4").expect("命令编号合写法")),
        body: TransientBody::CompactionDone(CompactionDone {
            seen: Seq::new(240).expect("240 是合法的序号"),
            trigger: CompactTrigger::Overflow,
            before: 1_012_000,
            after: 42_100,
            usage: Some(Usage {
                uncached: 905_400,
                cache_read: 0,
                cache_write: 0,
                output: 3560,
                reasoning: None,
            }),
            duration_ms: Some(52_300),
            prepared: false,
            truncated: Some(2),
        }),
    };
    assert_eq!(
        lines("transient/compaction.done.jsonl"),
        [done.to_line(), prepared.to_line(), truncated.to_line()]
    );
}

#[test]
fn the_todos_changed_sample_is_written_exactly() {
    let todo = |content: &str, status: TodoStatus| Todo {
        content: content.to_string(),
        status,
    };
    let changed = Transient {
        at: Timestamp::parse("2026-09-25T07:31:00.000Z").expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(65).expect("65 是合法的序号"))),
        by: By::Kernel,
        cause: None,
        body: TransientBody::TodosChanged(TodosChanged {
            todos: vec![
                todo("读代码", TodoStatus::Completed),
                todo("写测试", TodoStatus::InProgress),
                todo("跑门禁", TodoStatus::Pending),
            ],
            done: Vec::new(),
        }),
    };
    // 因为全部做完而清空的那一条带着做完的那一份（施工 D-3 补）。
    let cleared = Transient {
        body: TransientBody::TodosChanged(TodosChanged {
            todos: Vec::new(),
            done: vec![
                todo("读代码", TodoStatus::Completed),
                todo("写测试", TodoStatus::Completed),
            ],
        }),
        ..changed.clone()
    };
    assert_eq!(
        lines("transient/todos.changed.jsonl"),
        [changed.to_line(), cleared.to_line()]
    );
}

/// 包的会话状态变了（施工 TW-1）：一份给头看的（两块界面块的内容，一块换了标题），清掉了的没有。
#[test]
fn the_package_state_changed_sample_is_written_exactly() {
    let raw = |value: serde_json::Value| RawJson::of(&value).expect("写得出");
    let changed = Transient {
        at: Timestamp::parse("2026-10-28T09:12:00.000Z").expect("样本的时刻合写法"),
        turn: Some(TurnId::new(Seq::new(65).expect("65 是合法的序号"))),
        by: By::Kernel,
        cause: None,
        body: TransientBody::PackageStateChanged(PackageStateChanged {
            package: "todowrite".to_string(),
            state: State {
                widgets: Some(raw(serde_json::json!({
                    "list": {"content": [{"type": "list", "items": [
                        {"title": "读代码", "state": "done"},
                        {"title": "写测试", "state": "doing"},
                    ]}], "title": {"en": "Todos 1/2", "zh": "待办 1/2"}},
                    "count": {"content": [{"type": "text", "text": "1/2"}]},
                }))),
                ..State::default()
            },
        }),
    };
    let cleared = Transient {
        body: TransientBody::PackageStateChanged(PackageStateChanged {
            package: "todowrite".to_string(),
            state: State::default(),
        }),
        ..changed.clone()
    };
    assert_eq!(
        lines("transient/package.state_changed.jsonl"),
        [changed.to_line(), cleared.to_line()]
    );
}

/// 后台任务 `j1` 下面的第一次 `miyu call` 派去跑了（施工 M-1）：后台的不带回合、没有 `cause`，`by` 是内核。
#[test]
fn the_nested_started_sample_is_written_exactly() {
    let started = Transient {
        at: Timestamp::parse("2026-09-25T07:52:40.062Z").expect("样本的时刻合写法"),
        turn: None,
        by: By::Kernel,
        cause: None,
        body: TransientBody::NestedStarted(NestedStarted {
            id: NestedId::parse("j1#1").expect("样本里的嵌套调用编号合写法"),
            name: "kb_search".to_string(),
        }),
    };
    assert_eq!(
        lines("transient/tool.nested_started.jsonl"),
        [started.to_line()]
    );
}

/// 人要求的重起标题到了总时限没说完（施工 TT-3）：不带回合、没有 `cause`，`by` 是要它的人，给人看的那一句照 `human.get`。
#[test]
fn the_retitle_failed_sample_is_written_exactly() {
    let failed = Transient {
        at: Timestamp::parse("2026-11-13T08:20:11.250Z").expect("样本的时刻合写法"),
        turn: None,
        by: By::Person(Person::default()),
        cause: None,
        body: TransientBody::RetitleFailed(RetitleFailed {
            error: CallError {
                class: ErrorClass::Retryable,
                message: "总超时：180 秒还没有说完".to_string(),
                status: None,
                retry_after_ms: None,
            },
            human: Said::new("core/sessions/retitle-failed"),
        }),
    };
    let sample = lines("transient/session.retitle_failed.jsonl");
    assert_eq!(sample, [failed.to_line()]);
}
