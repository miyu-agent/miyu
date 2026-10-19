//! 脚本调工具（设计 39，原型 M-0）：执行器这一头的端口。只为实测省不省，走捷径：
//!
//! - 不进内核、不记事件、不排队：照这一次 `shell` 调用抄好的级别、工作目录当场判（`Guard::judge`，放行过的规则不带），
//!   放行的照同一个 `Tool::run` 当场跑。
//! - 判出要问人的不问，退出码 4（设计 39 MC4）。
//! - 能调的：设计 39 第三节出厂开的五件，加上 MCP；访问类别只准读、联网、执行。
//!
//! 正式的做法（内核的嵌套调用、`tool.nested`、账本）见设计 39 第四节。

use std::sync::Arc;

use miyu_kernel::block::Block;
use miyu_kernel::event::Permission;
use miyu_kernel::session::Verdict;
use miyu_kernel::tool::{Access, repair};
use miyu_tool::{Call, Calling, Done, MCP_PREFIX, NestedPort, Outcome, Progress, Shelf, Stop, Tool};

use crate::guard::Guard;
use crate::sandbox::Sandbox;

/// 出厂开给脚本的（设计 39 第三节第 2 条）。
const OPEN: &[&str] = &[
    "web_search",
    "web_fetch",
    "memory_search",
    "kb_search",
    "kb_read",
];

/// 一次 `shell` 调用的端口。
pub(super) struct Nester {
    pub(super) catalog: Shelf,
    pub(super) guard: Arc<Guard>,
    pub(super) sandbox: Option<Sandbox>,
    /// 照 `shell` 那一次抄好的调用：工作目录、各个端口；参数、旗、沙盒每次换。
    pub(super) template: Call,
    pub(super) permission: Permission,
    pub(super) dirs: Vec<String>,
}

impl Nester {
    /// 能不能从脚本调：名字在名单上（或是 MCP 的），访问类别是读、联网、执行。
    fn callable(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tool = self.catalog.get(name)?;
        let listed = OPEN.contains(&name) || name.starts_with(MCP_PREFIX);
        let access = &tool.spec().access;
        let kind = matches!(access, Access::Read | Access::Network | Access::Execute);
        (listed && kind).then_some(tool)
    }
}

/// 能调的名单，写进报错里。
fn listed() -> String {
    format!("{}, mcp__*", OPEN.join(", "))
}

impl NestedPort for Nester {
    fn call(&self, name: String, args: String) -> Calling {
        let Some(tool) = self.callable(&name) else {
            let why = match self.catalog.get(&name) {
                None => format!("No tool named {name}."),
                Some(_) => format!(
                    "miyu call does not work for {name}; call it directly. miyu call works only for: {}.",
                    listed()
                ),
            };
            return Box::pin(async move { Outcome::refused(2, why) });
        };
        let args = match repair(&tool.spec().parameters, &args) {
            Ok(args) => args,
            Err(_) => {
                let why = format!(
                    "Bad args for {name}: not a JSON object. See: miyu call --schema {name}"
                );
                return Box::pin(async move { Outcome::refused(2, why) });
            }
        };
        let guard = Arc::clone(&self.guard);
        let sandbox = self.sandbox.clone();
        let permission = self.permission.clone();
        let dirs = self.dirs.clone();
        let mut call = self.template.clone();
        call.args = args;
        call.stop = Stop::default();
        call.sandbox = None;
        call.nested = None;
        Box::pin(async move {
            let cwd = call.cwd.clone();
            let judged = {
                let (name, args, permission, dirs) =
                    (name.clone(), call.args.clone(), permission.clone(), dirs.clone());
                tokio::task::spawn_blocking(move || {
                    guard.judge(&name, args, cwd, &dirs, &permission, &[])
                })
                .await
            };
            match judged {
                Ok(Verdict::Allow) => {}
                Ok(Verdict::Deny { text, .. }) => return Outcome::refused(1, text),
                Ok(Verdict::Ask { .. }) => {
                    return Outcome::refused(
                        4,
                        format!(
                            "This call needs approval, which a script cannot ask for. Call {name} directly."
                        ),
                    );
                }
                Err(_) => return Outcome::refused(1, "Miyu could not check this call."),
            }
            if let Some(sandbox) = sandbox.filter(|_| !tool.outside_sandbox(&call)) {
                let cwd = call.cwd.clone();
                match tokio::task::spawn_blocking(move || sandbox.for_call(&permission, &cwd, &dirs))
                    .await
                {
                    Ok(Ok(sandboxed)) => call.sandbox = sandboxed.map(Arc::new),
                    _ => return Outcome::refused(1, "Miyu could not set up the sandbox."),
                }
            }
            let done = tool.run(call, Progress::new(|_| {})).await;
            outcome(done)
        })
    }

    fn schema(&self, name: &str) -> Option<String> {
        let tool = self.callable(name)?;
        let spec = tool.spec();
        Some(format!(
            "{name}: {}\nParameters: {}\n",
            spec.description,
            spec.parameters.get()
        ))
    }
}

/// 工具交回的换成给脚本的：出错的退出码 1、字写进标准错误；有结构化结果的印一行 JSON；别的印字。
fn outcome(done: Done) -> Outcome {
    let text = done
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut stderr = String::new();
    if !done.images.is_empty() {
        stderr.push_str("Images are not passed to scripts.\n");
    }
    if done.error {
        stderr.push_str(&line(text));
        return Outcome {
            code: 1,
            stdout: String::new(),
            stderr,
        };
    }
    let stdout = match done.detail {
        Some(detail) => line(detail.to_string()),
        None => line(text),
    };
    Outcome {
        code: 0,
        stdout,
        stderr,
    }
}

/// 不是空的，就以换行结尾。
fn line(mut text: String) -> String {
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text
}
