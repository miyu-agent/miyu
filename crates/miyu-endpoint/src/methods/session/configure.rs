//! `session.configure`（施工 L-2 那时在 `session.rs`，施工 CT-1 那边放不下了挪出来）：换模型、思考强度、上下文上限、看图模型、
//! 压缩触发线、人格、预设。

use super::super::*;
use super::recast;

/// `session.configure`：换模型、看图模型（施工 VM-1，`models/vision.rs`）、压缩触发线（施工 CT-1，`models/line.rs`）；写 `null`
/// 的回到跟着配置。换思考强度（施工 SM-1）照会话这时的模型查（`models/effort.rs`），上下文上限（施工 CTX-1）照压缩线查
/// （`models/context.rs`）。换人格、预设（施工 P-13）在`recast.rs`：先查完，模型先换，再换思考强度、上下文上限、看图模型、
/// 压缩触发线，再换人格、预设。
pub(in crate::methods) async fn configure(
    core: &Arc<Core>,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: models::ConfigureParams = params(request)?;
    let text = params.model()?;
    let session = session(&params.session)?;
    let recast = recast::checked(core, &session, params.recast()).await?;
    let found = core.sessions.get(core, &session).await?;
    // 写 `null` 的回到跟着 `models.chat`：内核记空的（施工 8-10 再补）。
    let model = match text {
        Some(Some(text)) => Some(models::record(core, text)?),
        Some(None) => Some(String::new()),
        None => None,
    };
    let effort = match params.effort() {
        Some(level) => {
            Some(models::effort::command(core, &found.handle, model.as_deref(), level).await?)
        }
        None => None,
    };
    let context = match params.context() {
        Some(tokens) => Some(models::context::command(&found.handle, tokens).await?),
        None => None,
    };
    let vision = models::vision::command(core, params.vision())?;
    let line = models::line::command(params.compaction_line())?;
    if let Some(model) = model {
        let command = Command::Configure { model };
        command_to(core, request, &session, &found.handle, command).await?;
    }
    let to = (&session, &found.handle);
    let commands = [
        ("effort", effort),
        ("context", context),
        ("vision", vision),
        ("compaction_line", line),
    ];
    for (suffix, command) in commands {
        if let Some(command) = command {
            recast::send_as(core, request, to, suffix, command).await?;
        }
    }
    if let Some(command) = recast {
        recast::send(core, request, &session, &found.handle, command).await?;
        // 换人格、换预设已经先压缩过（施工 PX-1）：这一刻以前的改动不再问第二次。记成放行、不记成看过：回合开始照旧照现在的
        // 文件重拼，改过的覆盖照样换上。快照等下一个回合才换，会话照这一刻的文件记下那一份当基准（施工 PX-1补）。
        found.handle.settle_prefix(core.edits.version());
    }
    Ok(json!({}))
}
