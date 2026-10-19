//! 握手以后的方法（`docs/designs/04-核心协议.md` 第九节「先做的几样怎么写」）：造会话、说话、打断，
//! 列出会话（施工 3-9 下），撤销、恢复（施工 4-7 上；回应带上给人看的几样，施工 4-7 下），重做（施工 4-7 再补），手动压缩
//! （施工 6-8），切权限级别（施工 3-8 再补），清空上下文（施工 6-8 补），回顾（施工 3-8 四补，回应是那一句），停掉一个任务（施工 7-4），读后台命令的输出（施工 7-4 补），
//! 传附件（施工 3-9 三补），改标题、置顶，删除会话（施工 3-8 三补）。别的 harness 带着名字说话（`session.send` 的 `from`，
//! 施工 7-10）。命令交给会话，等它的回应：接受的回 `events`（切权限级别、停掉任务、改标题的回 `{}`），拒绝的回原因码；删除
//! 由会话表办。造会话、说话的回应再带上会话实际在哪个目录里干活（施工 4-5 下），这个目录的项目配置还没问过信不信任的，
//! 再带上它在哪（`untrusted_project`，施工 8-2）。查配置的三个方法在 `config/methods.rs`（施工 8-2），改配置的 `config.set`、
//! 信任项目配置的 `config.trust` 在 `config/set.rs`、`config/trusting.rs`（施工 8-3）。密钥的 `secret.set`、`secret.delete`、
//! `secret.list` 在 `secrets.rs`（施工 8-5）。`model.list` 在 `models.rs`（施工 8-7）。`session.create` 带 `model` 的照这时的
//! 配置解析好再造，解析不出的回 `unknown_model`、什么都不造（施工 8-8，`models::record`）。换模型 `session.configure`：先查
//! 参数、再找会话、再照这时的配置解析，交给内核，回 `{}`（施工 8-10）。给人看的字 `human.get` 在 `human.rs`（施工 W-1）。
//! 列文件、找文件、换真实位置、分块读 `fs.list`、`fs.find`、`fs.realpath`、`fs.read` 在 `files.rs`
//! （施工 W-2、W-3、W-6）。第一次接入的
//! `provider.detect`、`provider.catalog`、`provider.test` 在 `providers.rs`（施工 8-11）。分块上传
//! `blob.open`、`blob.write`、`blob.close` 在 `uploads.rs`（施工 W-5），要这个连接的上传表 `uploads`。分块读一个 blob
//! `blob.get` 在 `attach.rs`（施工 W-6）。用量汇总的 `usage.query` 在 `usage.rs`（施工 8-15）。
//! 各方法的参数在 `methods/params.rs`（W-5 合并时这一份过了 500 行，挪出去的）。会话的那十几个方法的身子在 `methods/session.rs`，这里只路由。

use std::pin::Pin;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Level;
use miyu_kernel::id::{CallId, JobId, Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome, Queued};
use miyu_policy::memory::MemoryScope;
use miyu_session::Handle;

use crate::Core;
use crate::attach;
use crate::commands;
use crate::config;
use crate::files;
use crate::from;
use crate::hello::{Caller, Peer};
use crate::human;
use crate::job_output;
use crate::list;
use crate::memory;
use crate::meta::MetaParams;
use crate::models;
use crate::personas;
use crate::providers;
use crate::refusal::Refusal;
use crate::secrets;
use crate::sessions::{Opening, admin};
use crate::undo;
use crate::uploads::{self, Uploads};
use crate::venues;
use crate::wire::Request;

mod later;
mod params;
mod session;

pub(crate) use later::{answered_later, call_later};
use params::*;

/// 照方法办一条请求：交回办它的 future（[`answer`] 放在堆上），出来的是回应的 `result`，或者拒绝。
pub(crate) fn call<'a>(
    core: &'a Arc<Core>,
    peer: Peer,
    caller: &'a Caller,
    request: &'a Request,
    uploads: &'a mut Uploads,
) -> Answer<'a> {
    match request.method.as_str() {
        "session.create" => answer(move || session::create(core, peer, caller, request)),
        "check" => {
            answer(move || async move { crate::check::check(core, peer, params(request)?).await })
        }
        "persona.list" => answer(move || async move { personas::list(core, peer).await }),
        "persona.avatar" => answer(move || async move {
            personas::picture::read(core, personas::picture::Picture::Avatar, params(request)?)
                .await
        }),
        "persona.background" => answer(move || async move {
            personas::picture::read(
                core,
                personas::picture::Picture::Background,
                params(request)?,
            )
            .await
        }),
        // 软件包的几个方法（施工 F-8 中上挪出去：这里放不下了）。
        name if ["package.", "mcp.", "widget."]
            .iter()
            .any(|at| name.starts_with(at)) =>
        {
            answer(move || async move {
                crate::packages::methods::call(core, peer, caller, request).await
            })
        }
        "project.list" | "project.create" | "project.update" | "project.delete" => {
            answer(move || async move { crate::projects::call(core, caller, request).await })
        }
        "view.page" => {
            answer(move || async move { crate::view::page(core, peer, params(request)?).await })
        }
        "view.detail" => {
            answer(move || async move { crate::view::detail(core, params(request)?).await })
        }
        "extension.status" => {
            answer(move || async move { Ok(crate::extensions::status(core, peer)) })
        }
        "extension.enable" => {
            answer(
                move || async move { crate::extensions::enable(core, peer, params(request)?).await },
            )
        }
        "extension.disable" => {
            answer(
                move || async move { crate::extensions::disable(core, peer, params(request)?).await },
            )
        }
        "extension.restart" => {
            answer(
                move || async move { crate::extensions::restart(core, peer, params(request)?).await },
            )
        }
        "persona.get" => {
            answer(move || async move { personas::get(core, peer, params(request)?).await })
        }
        "persona.set" => {
            answer(move || async move { personas::write::set(core, peer, params(request)?).await })
        }
        "persona.read" => answer(move || async move {
            personas::write::read_prompt(core, peer, params(request)?).await
        }),
        "persona.delete" => {
            answer(move || async move { personas::write::delete(core, params(request)?).await })
        }
        "preset.list" => answer(move || async move { crate::presets::list(core, peer).await }),
        "preset.get" => {
            answer(move || async move { crate::presets::get(core, peer, params(request)?).await })
        }
        "preset.set" => {
            answer(
                move || async move { crate::presets::write::set(core, peer, params(request)?).await },
            )
        }
        "preset.delete" => {
            answer(
                move || async move { crate::presets::write::delete(core, params(request)?).await },
            )
        }
        kb if kb.starts_with("kb.") => {
            answer(move || async move { crate::knowledge::call(core, kb, &request.params).await })
        }
        method if memory::METHODS.contains(&method) => answer(move || async move {
            memory::call(core, &request.method, &request.id, &request.params).await
        }),
        "session.list" => answer(move || async move {
            let params: ListParams = params(request)?;
            let sessions = list::list(core, params.oneshot, params.limit).await?;
            Ok(json!({"sessions": sessions}))
        }),
        "events.append" => answer(move || async move {
            crate::appending::append(core, caller, request, params(request)?).await
        }),
        "model.embed" => {
            answer(move || async move { crate::embedding::embed(core, params(request)?).await })
        }
        "content.list" => {
            answer(move || async move { crate::content::list(core, caller, params(request)?) })
        }
        "content.read" => {
            answer(
                move || async move { crate::content::read(core, caller, params(request)?).await },
            )
        }
        "system.append" => {
            answer(move || async move { crate::appends::append(core, caller, params(request)?) })
        }
        "provide" => {
            answer(
                move || async move { crate::provide::provide(core, caller, params(request)?).await },
            )
        }
        "session.respond" => answer(move || async move {
            crate::responding::respond(core, caller, request, params(request)?).await
        }),
        "session.fork" => answer(move || async move {
            crate::forking::fork(core, caller, request, params(request)?).await
        }),
        "session.note" => answer(move || async move {
            crate::responding::note(core, caller, request, params(request)?).await
        }),
        "venue.records" => {
            answer(move || async move { venues::records(core, params(request)?).await })
        }
        "venue.binding" => {
            answer(move || async move { venues::binding(core, &caller.account, params(request)?) })
        }
        "venue.session" => answer(move || async move {
            venues::session(core, &caller.account, request.id.clone(), params(request)?).await
        }),
        "venue.sessions" => answer(move || async move {
            let params: venues::SessionsParams = params(request)?;
            venues::sessions(core, &caller.account, params).await
        }),
        "session.send" => answer(move || session::send(core, request)),
        "session.interrupt" => answer(move || session::interrupt(core, request)),
        "session.revert" => answer(move || session::revert(core, request)),
        "session.unrevert" => answer(move || session::unrevert(core, request)),
        "command.run" => answer(move || async move {
            commands::run(core, &peer, &request.id, params(request)?).await
        }),
        "command.catalog" => {
            answer(move || async move { commands::catalog(core, &peer, params(request)?).await })
        }
        "session.redo" => answer(move || session::redo(core, request)),
        "session.compact" => answer(move || session::compact(core, request)),
        "session.set_permission_level" => {
            answer(move || session::set_permission_level(core, request))
        }
        "session.answer" => answer(move || session::answer_question(core, request)),
        "session.clear" => answer(move || session::clear(core, request)),
        "session.recap" => answer(move || session::recap(core, request)),
        "job.stop" => answer(move || session::stop_job(core, request)),
        "job.output" => {
            answer(move || async move { job_output::read(core, params(request)?).await })
        }
        "human.get" => {
            answer(move || async move { human::get(core, peer, params(request)?).await })
        }
        "config.schema" => {
            answer(move || async move { config::methods::schema(core, peer, params(request)?) })
        }
        "config.get" => {
            answer(move || async move { config::methods::get(core, peer, params(request)?) })
        }
        "config.check" => {
            answer(move || async move { config::methods::check(core, peer, params(request)?) })
        }
        "config.set" => answer(move || async move {
            let params = params(request)?;
            match &caller.package {
                // 核心拉起的扩展只能改自己的几项（施工 T-9 补），记成这个包。
                Some(package) => {
                    let by = crate::appending::recorder(core, caller)?;
                    config::set::set_from_package(core, peer, &request.id, params, (package, by))
                }
                None => config::set::set(core, peer, &request.id, params),
            }
        }),
        "config.trust" => answer(move || async move {
            config::trusting::trust(core, peer, &request.id, params(request)?)
        }),
        "secret.set" => {
            answer(move || async move { secrets::set(core, peer, &request.id, params(request)?) })
        }
        "secret.delete" => {
            answer(
                move || async move { secrets::delete(core, peer, &request.id, params(request)?) },
            )
        }
        "secret.list" => answer(move || async move { Ok(secrets::list(core)) }),
        "model.list" => answer(move || async move { models::list(core, params(request)?).await }),
        "provider.detect" => answer(move || async move { providers::detect(core).await }),
        "provider.catalog" => {
            answer(move || async move { providers::catalog(core, peer, params(request)?).await })
        }
        "provider.test" => {
            answer(move || async move { providers::test(core, peer, params(request)?).await })
        }
        "usage.query" => {
            answer(move || async move { crate::usage::query(core, params(request)?).await })
        }
        "blob.put" => answer(move || async move {
            attach::put(core, caller.package.as_deref(), params(request)?).await
        }),
        "blob.open" => {
            answer(move || async move { uploads::open(core, uploads, params(request)?).await })
        }
        "blob.write" => {
            answer(move || async move { uploads::write(core, uploads, params(request)?).await })
        }
        "blob.close" => answer(move || async move {
            uploads::close(core, (uploads, caller.package.as_deref()), params(request)?).await
        }),
        "blob.get" => answer(move || async move {
            attach::get(
                core,
                (&caller.account, caller.package.as_deref()),
                params(request)?,
            )
            .await
        }),
        "session.configure" => answer(move || session::configure(core, request)),
        "fs.list" => answer(move || async move { files::list(core, params(request)?).await }),
        "fs.find" => answer(move || async move { files::find(core, params(request)?).await }),
        "fs.realpath" => {
            answer(move || async move { files::realpath(core, params(request)?).await })
        }
        "fs.read" => answer(move || async move { files::read(core, params(request)?).await }),
        "session.set_workspace" => {
            answer(
                move || async move { crate::workspace::set(core, request, params(request)?).await },
            )
        }
        "session.set_meta" => answer(move || session::set_meta(core, caller, request)),
        "session.terminal" => answer(move || async move {
            crate::terminal::get(core, peer, &caller.account, request, params(request)?).await
        }),
        "session.set_terminal" => answer(move || async move {
            crate::terminal::set(core, &caller.account, params(request)?).await
        }),
        "session.delete" => answer(move || session::delete(core, request)),
        other => answer(move || async move {
            match core.queries.get(other).filter(|_| core.serves(other)) {
                // 可选软件包登记的查询（施工 W-4，`queries.rs`）：没登记的方法，这张表之外当没有这个方法。
                Some(handler) => handler(Arc::clone(core), request.params.clone())
                    .await
                    .map_err(Refusal::from),
                None => Err(Refusal::UNKNOWN_METHOD),
            }
        }),
    }
}

/// 一条请求在办的样子：放在堆上。
pub(crate) type Answer<'a> = Pin<Box<dyn Future<Output = Result<Value, Refusal>> + Send + 'a>>;

/// 照 `make` 造出办这条请求的 future，放到堆上。造在这个函数自己的一帧里：调试版不复用栈槽，八十来个方法的 future 写在
/// 一个 async 函数里，每个都在它那一帧里占一份，一帧上 1 MB，Windows 上的调试版跟着请求往下走就栈溢出了（施工 L-2 时
/// 撞上的）。
fn answer<'a, F, Fut>(make: F) -> Answer<'a>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Value, Refusal>> + Send + 'a,
{
    Box::pin(make())
}

/// 把命令交给会话 `session`（把手是 `handle`），记成管理员发的，等它的回应：接受的交回它产生的事件的序号。会话停了的
/// 从表里拿掉。
pub(crate) async fn command_to(
    core: &Core,
    request: &Request,
    session: &SessionId,
    handle: &Handle,
    command: Command,
) -> Result<Vec<u64>, Refusal> {
    command_by(core, request, session, handle, admin(core), command).await
}

/// 同 [`command_to`]，记成 `by` 发的：别的 harness 发来的话（施工 7-10）。
pub(crate) async fn command_by(
    core: &Core,
    request: &Request,
    session: &SessionId,
    handle: &Handle,
    by: By,
    command: Command,
) -> Result<Vec<u64>, Refusal> {
    match outcome(core, request, session, handle, by, command).await? {
        Outcome::Accepted { events } => Ok(events.iter().map(|seq| seq.get()).collect()),
        // 只有回顾回它（施工 3-8 四补），回顾不走这里。
        _ => Err(Refusal::INTERNAL),
    }
}

/// 把命令交给会话，记成 `by` 发的，交回它的结局：拒绝的是内核的原因码；会话停了的从表里拿掉。
async fn outcome(
    core: &Core,
    request: &Request,
    session: &SessionId,
    handle: &Handle,
    by: By,
    command: Command,
) -> Result<Outcome, Refusal> {
    match handle.command(request.id.clone(), by, command).await {
        Ok(Outcome::Rejected { reason, about }) => Err(Refusal::kernel_about(reason, &about)),
        Ok(outcome) => Ok(outcome),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 人说的一句话写成内容块：照原样成一块文字，空的一块都没有（`session.send`、`session.redo` 的 `text`）。
fn said(text: String) -> Vec<Block> {
    match text.is_empty() {
        true => Vec::new(),
        false => vec![Block::Text(Text { text })],
    }
}

/// 读参数；读不成的是参数不对。
pub(crate) fn params<T: serde::de::DeserializeOwned>(request: &Request) -> Result<T, Refusal> {
    serde_json::from_value(request.params.clone()).map_err(|_| Refusal::BAD_PARAMS)
}

/// 会话编号；不合写法的是参数不对。
fn session(text: &str) -> Result<SessionId, Refusal> {
    SessionId::parse(text).map_err(|_| Refusal::BAD_PARAMS)
}
