//! 拒绝时给人看的话（`docs/designs/04-核心协议.md` 第六节第 4 条）：一个原因码一句中文、一句英文。施工 W-8 从
//! `refusal.rs` 挪出来（那一份过了 500 行）。

use std::borrow::Cow;

use serde_json::{Map, Value};

/// 一句拒绝的中文、英文：照 [`of`]，只是带着 `data` 里的数说的几种照它填（施工 YC-6：`session_read_only` 照 `data.line` 说坏在
/// 第几行；`data` 里没有那个数的照 [`of`]）。
pub(super) fn said(
    reason: &str,
    data: Option<&Map<String, Value>>,
) -> (Cow<'static, str>, Cow<'static, str>) {
    let line = data
        .and_then(|data| data.get("line"))
        .and_then(Value::as_u64);
    match (reason, line) {
        ("session_read_only", Some(line)) => (
            format!(
                "会话日志第 {line} 行坏了，只能看到这一行以前的内容，不能接着聊。要继续请新开一个会话。"
            )
            .into(),
            format!(
                "The session log is broken at line {line}; only the part before it can be viewed and the session can't continue. Start a new session to go on."
            )
            .into(),
        ),
        _ => {
            let (zh, en) = of(reason);
            (zh.into(), en.into())
        }
    }
}

/// 原因码 `reason` 的中文、英文。不认识的说「已拒绝」。
pub(super) fn of(reason: &str) -> (&'static str, &'static str) {
    match reason {
        "parse_error" => ("消息格式错误。", "Malformed message."),
        "line_too_long" => (
            "消息超过 1 MiB，没有收。大的内容请用附件发。",
            "Message over 1 MiB, not accepted. Send large content as an attachment.",
        ),
        "invalid_request" => ("无效的请求。", "Invalid request."),
        "unknown_method" => ("方法不存在。", "Method not found."),
        "bad_params" => ("参数错误。", "Invalid parameters."),
        // 施工 YC-7（`protocol.md`「同一个命令编号」）。
        "id_reused" => (
            "同一个请求编号换了参数，没有收。请换一个新的编号。",
            "This request id was already used with different parameters. Use a new id.",
        ),
        "internal_error" => (
            "内部错误，详见运行日志。",
            "Internal error. See the runtime log.",
        ),
        "hello_first" => ("请先握手（hello）。", "Send hello first."),
        "protocol_mismatch" => (
            "协议版本不兼容，请升级到同一版本。",
            "Protocol version mismatch. Upgrade to the same release.",
        ),
        "bad_token" => ("本机令牌无效。", "Invalid local token."),
        "unknown_persona" => ("人格不存在。", "Persona not found."),
        "persona_invalid" => ("人格文件有误。", "Invalid persona files."),
        "unknown_preset" => ("预设不存在。", "Preset not found."),
        "preset_invalid" => ("预设文件有误。", "Invalid preset file."),
        "not_a_directory" => ("不是目录。", "Not a directory."),
        "unknown_package" => ("软件包不存在。", "Package not found."),
        "unknown_source" => ("没有这个软件源。", "No such package source."),
        "unknown_project" => ("项目不存在。", "Project not found."),
        "not_a_ui_package" => ("不是界面软件包。", "Not a UI package."),
        "prefs_too_big" => ("设置超过 64 KiB。", "Settings over 64 KiB."),
        "export_exists" => ("文件已存在。", "File already exists."),
        "export_in_data_root" => (
            "不能导出到数据目录内。",
            "Cannot export into the data root.",
        ),
        "export_unwritable" => ("无法写入该位置。", "Cannot write there."),
        "export_too_big" => (
            "导出超过 512 KiB，请存成文件。",
            "The export is over 512 KiB. Save it to a file.",
        ),
        "bad_bundle" => ("不是会话包。", "Not a session bundle."),
        "bundle_mismatch" => ("会话包已损坏。", "The session bundle is damaged."),
        "bundle_too_new" => (
            "会话包来自更新的版本。",
            "The bundle is from a newer version.",
        ),
        "session_exists" => (
            "已有同编号的会话。",
            "A session with this id already exists.",
        ),
        "archived" => (
            "这是存档，不能继续对话。",
            "This is an archive. It can't be continued.",
        ),
        "unknown_server" => (
            "设置里没有这个 MCP 服务。",
            "No such MCP server in the settings.",
        ),
        "oauth_unregistered" => (
            "这个服务要先在它那边注册客户端。",
            "Register a client with this server first.",
        ),
        "oauth_failed" => ("登录没成。", "Login failed."),
        "not_logged_in" => ("这个服务没登录过。", "This server is not logged in."),
        "not_waiting" => (
            "这个服务没有在等的登录，先发起登录。",
            "No login is waiting for this server; start one first.",
        ),
        "server_from_package" => (
            "这个 MCP 服务随软件包提供，不能在这里修改。",
            "This MCP server comes with a package and cannot be changed here.",
        ),
        "mcp_invalid" => ("MCP 服务写法有误。", "Invalid MCP server."),
        "unknown_skill" => ("技能不存在。", "Skill not found."),
        "skill_from_package" => (
            "这个技能随软件包提供，不能删除。",
            "This skill comes with a package and cannot be deleted.",
        ),
        "skill_shared" => (
            "这个技能在别的目录里，别的应用也在用，请到那里删除。",
            "This skill is in another folder, shared with other apps; delete it there.",
        ),
        "package_exists" => (
            "编号与内置软件包重复。",
            "Id conflicts with a built-in package.",
        ),
        "package_invalid" => ("清单有误，无法安装。", "Invalid manifest, cannot install."),
        "package_file_bad" => ("包文件无法解开。", "Cannot unpack the package file."),
        "download_failed" => ("软件包下载失败。", "Package download failed."),
        "source_unreachable" => ("软件源连不上。", "Package source unreachable."),
        "wrong_target" => ("不支持本机的系统。", "Not available for this system."),
        "package_conflict" => (
            "名称已被占用，无法安装。",
            "Names already in use, cannot install.",
        ),
        "program_missing" => ("程序未安装。", "Program not installed."),
        "depends_missing" => ("缺少依赖的软件包。", "Required packages are missing."),
        "package_depended" => (
            "其他软件包依赖它，无法卸载。",
            "Other packages depend on it.",
        ),
        "no_page" => ("没有后台页。", "No admin page."),
        "not_found" => ("文件不存在。", "File not found."),
        "program_not_running" => ("程序未运行。", "Program not running."),
        "no_drivers" => (
            "这个软件包没有模型驱动。",
            "This package provides no model driver.",
        ),
        "driver_busy" => (
            "有会话正在用它的模型驱动，等它们说完再查。",
            "A session is using its model driver; check again when it is done.",
        ),
        "method_timeout" => ("方法超时。", "Method timed out."),
        "unregistered" => ("方法未登记。", "Method not registered."),
        "no_provider" => (
            "总线上没有提供这个名字的软件包。",
            "No package provides this name on the bus.",
        ),
        "package_required" => (
            "这个名字有几个软件包接着，要写用哪一个。",
            "Several packages plug into this name; say which one.",
        ),
        "own_package_only" => (
            "扩展照包编号只能调自己，调别的软件包请用 bus.call。",
            "An extension can call only itself by package id; use bus.call for other packages.",
        ),
        "method_failed" => ("方法执行失败。", "Method failed."),
        "avatar_not_image" => (
            "头像只支持 PNG、JPEG、WebP。",
            "Avatars must be PNG, JPEG or WebP.",
        ),
        "avatar_too_big" => (
            "头像超过 1 MiB 或 1024 像素。",
            "Avatar over 1 MiB or 1024 pixels.",
        ),
        "background_not_image" => (
            "背景图只支持 PNG、JPEG、WebP。",
            "Backgrounds must be PNG, JPEG or WebP.",
        ),
        "background_too_big" => (
            "背景图超过 5 MiB 或 4096 像素。",
            "Background over 5 MiB or 4096 pixels.",
        ),
        "not_switchable" => ("不能启用或停用。", "Cannot be enabled or disabled."),
        "not_restartable" => ("不能重启。", "Cannot be restarted."),
        "package_off" => ("软件包已停用。", "Package disabled."),
        "not_an_extension" => ("不是扩展。", "Not an extension."),
        "extension_off" => ("扩展已停用。", "Extension disabled."),
        "port_taken" => ("端口已被占用。", "Port in use."),
        "prefix_changing" => (
            "检测到文件变动，继续对话会因缓存失效多付输入费用。",
            "Files changed; continuing will cost extra input because the cache is invalidated.",
        ),
        "needs_approval" => ("扩展权限未批准。", "Extension permissions not approved."),
        "session_not_found" => ("会话不存在。", "Session not found."),
        "unknown_call" => ("调用不存在。", "Call not found."),
        "shell_not_allowed" => (
            "只有你自己能在这里跑命令。",
            "Only you can run commands here.",
        ),
        "shell_unavailable" => (
            "没有能跑命令的工具（软件包没装或已关闭）。",
            "No tool can run commands (package missing or disabled).",
        ),
        "venue_session" => (
            "平台会话不能直接发消息。",
            "Cannot send messages directly to a platform session.",
        ),
        "unknown_command" => ("命令不存在。", "Command not found."),
        "command_unavailable" => ("命令暂时不可用。", "Command unavailable for now."),
        "command_failed" => ("命令执行失败。", "Command failed."),
        "unknown_file" => ("不支持检查这个文件。", "This file cannot be checked."),
        "command_not_allowed" => ("仅主人和群管理员可用。", "Owner and group admins only."),
        "nothing_to_delete" => ("没有可删除的内容。", "Nothing to delete."),
        "avatar_conflict" => (
            "头像已被修改，请刷新后重试。",
            "Avatar was modified. Refresh and try again.",
        ),
        "persona_conflict" => (
            "人格已被修改，请刷新后重试。",
            "Persona was modified. Refresh and try again.",
        ),
        "preset_conflict" => (
            "预设已被修改，请刷新后重试。",
            "Preset was modified. Refresh and try again.",
        ),
        "owner_only" => ("仅主人可用。", "Owner only."),
        "session_stopped" => (
            "会话已停止，重新发送即可载入。",
            "Session stopped. Send again to reload.",
        ),
        "session_broken" => (
            "会话已损坏，无法载入。",
            "Session is corrupted and cannot be loaded.",
        ),
        "session_unavailable" => (
            "会话暂时无法读取，请稍后重试。",
            "Session can't be read right now. Try again later.",
        ),
        // 施工 YC-6：带着行号说的在 [`said`]，这一句是 `data` 里没有行号时的。
        "session_read_only" => (
            "会话日志坏了，只能看到坏的地方以前的内容，不能接着聊。要继续请新开一个会话。",
            "The session log is broken; only the part before it can be viewed and the session can't continue. Start a new session to go on.",
        ),
        "empty_message" => ("消息为空。", "Empty message."),
        "dir_too_wide" => ("目录范围过大。", "Directory too broad."),
        "attachment_unreadable" => ("无法读取文件。", "Cannot read file."),
        "attachment_too_big" => (
            "附件过大（最大 20 MiB，图片 5 MiB、8000 像素）。",
            "Attachment too large (max 20 MiB; images 5 MiB, 8000 px).",
        ),
        "attachment_in_data_root" => (
            "不能附加数据目录里的文件。",
            "Files in the data directory cannot be attached.",
        ),
        "unknown_attachment" => ("附件不存在。", "Attachment not found."),
        // 施工 W-2（`web-module.md`「给人看的字」）。
        "path_unreadable" => ("无法读取路径。", "Cannot read path."),
        "path_forbidden" => ("无权访问。", "Access denied."),
        // 施工 W-4（`mermaid.md`「给人看的字」）。
        "mermaid_too_long" => ("图表源码过长。", "Diagram source too long."),
        "mermaid_failed" => ("图表渲染失败。", "Diagram rendering failed."),
        // 施工 W-5（`web-module.md`「给人看的字」）。
        "too_many_uploads" => ("同时上传的文件过多。", "Too many uploads at once."),
        "upload_unknown" => ("上传已失效，请重新上传。", "Upload expired. Upload again."),
        "upload_offset" => ("上传位置不一致。", "Upload offset mismatch."),
        "upload_incomplete" => ("上传未完成。", "Upload incomplete."),
        // 施工 W-6（`web-module.md`「给人看的字」）。
        "unknown_blob" => ("内容不存在。", "Content not found."),
        "not_running" => ("没有进行中的回合。", "No turn in progress."),
        // 施工 Q-W（`session.withdraw`）。
        "nothing_queued" => ("没有排队的消息。", "No queued messages."),
        "already_heard" => ("消息已送达，无法撤回。", "Message already delivered."),
        "turn_running" => ("回合进行中。", "A turn is in progress."),
        "unknown_turn" => ("回合不存在。", "Turn not found."),
        "nothing_to_unrevert" => ("没有可恢复的撤销。", "Nothing to restore."),
        "restoring" => ("正在撤销或恢复。", "Undo or restore in progress."),
        "nothing_to_revert" => ("没有可撤销的回合。", "Nothing to undo."),
        "memory_unavailable" => ("记忆不可用。", "Memory unavailable."),
        "memory_not_installed" => ("人格记忆未安装。", "Persona memory not installed."),
        "memory_busy" => ("正在整理记忆。", "Memory is being organized."),
        "dream_failed" => ("整理记忆失败。", "Memory organization failed."),
        "not_dreaming" => ("没有正在进行的整理。", "No memory organizing in progress."),
        "unknown_memory" => ("记忆不存在。", "Memory not found."),
        "memory_not_current" => ("记忆已失效。", "Memory no longer current."),
        "memory_too_long" => ("记忆过长。", "Memory too long."),
        "unknown_kb" => ("知识库不存在。", "Knowledge base not found."),
        "unknown_document" => ("文档不存在。", "Document not found."),
        // 施工 K-7 下：K-7 上加的五种（`knowledge.md` 第八条）。
        "kb_exists" => ("知识库已经有了。", "Knowledge base already exists."),
        "document_exists" => ("这一篇已经有了。", "Document already exists."),
        "kb_read_only" => (
            "挂进来的知识库是只读的。",
            "A mounted knowledge base is read-only.",
        ),
        // 施工 K-8：软件包带来的库。
        "kb_packaged" => (
            "随软件包来的知识库是只读的，只能开关自动想起。",
            "A knowledge base from a package is read-only; only auto recall can be switched.",
        ),
        "folder_unavailable" => ("这个目录挂不上。", "Folder cannot be mounted."),
        "trash_failed" => ("挪不进回收站。", "Could not move it to the trash."),
        "unknown_job" => ("任务不存在或已结束。", "Job not found or finished."),
        "not_a_command" => ("不是后台命令。", "Not a background command."),
        "nothing_to_compact" => ("没有可压缩的内容。", "Nothing to compact."),
        // 2026-09-30 项目主人定（施工 6-8 补）：头把它当一条提示通知显示。
        "nothing_to_clear" => ("上下文为空。", "Context is empty."),
        // 2026-09-30 项目主人定（施工 4-7 再补）：两种情况一句话，头把它当一条提示通知显示。
        "not_redoable" => ("无法重做。", "Cannot redo."),
        // 施工 O-14 上：`session.respond` 的两个。
        "not_ambient" => ("不是旁听消息。", "Not an overheard message."),
        // 施工 O-2 上：`provide` 的两个。
        "not_a_provider" => ("仅扩展可提供工具。", "Only extensions can provide tools."),
        "bad_tool" => ("工具规格有误。", "Invalid tool spec."),
        "already_answered" => ("已回复。", "Already answered."),
        "session_sealed" => ("会话已封存。", "Session sealed."),
        "goal_exists" => ("已有一个长期目标。", "A goal is already set."),
        "no_goal" => ("没有长期目标。", "No goal."),
        "goal_paused" => ("长期目标已暂停。", "Goal paused."),
        "goal_running" => ("长期目标正在进行。", "Goal running."),
        "goal_unavailable" => (
            "此会话不支持长期目标。",
            "Goals are not available in this session.",
        ),
        // 施工 D-1：`session.answer` 碰得到的四个（`unknown_decision` 协议上碰不到，`protocol.md`「出错」）。
        "not_asking" => ("不在等待回答。", "Not waiting for an answer."),
        "no_rule" => ("只能允许本次或拒绝。", "Only allow once or deny."),
        "unexpected_reason" => ("仅拒绝可附理由。", "Only a denial can carry a reason."),
        "bad_answer" => ("回答与题目不匹配。", "Answers do not match the questions."),
        // 2026-10-01 主会话定（施工 3-8 四补）。
        "nothing_to_recap" => ("没有可回顾的内容。", "Nothing to recap."),
        // 施工 8-2（`config.md`「协议拒绝时的话」）。
        "unknown_config_key" => ("配置项不存在。", "Config key not found."),
        // 施工 8-3（`config.md`「协议拒绝时的话」）。
        "config_invalid" => ("配置有误，未保存。", "Invalid config, not saved."),
        "config_conflict" => (
            "配置已被修改，请刷新后重试。",
            "Config was modified. Refresh and try again.",
        ),
        "config_file_broken" => (
            "配置文件有误，请先修复。",
            "Config file is invalid. Fix it first.",
        ),
        "no_project_config" => ("未找到项目配置。", "Project config not found."),
        "unknown_secret" => ("密钥不存在。", "Secret not found."),
        "unknown_provider" => ("供应商不存在。", "Provider not found."),
        "provider_exists" => ("供应商编号已存在。", "Provider ID already exists."),
        "preset_exists" => ("预设编号已存在。", "Preset ID already exists."),
        "persona_exists" => ("人格编号已存在。", "Persona ID already exists."),
        "unknown_model" => ("模型或模型池不存在。", "Model or pool not found."),
        "recap_failed" => ("生成回顾失败。", "Recap failed."),
        // 施工 8-20（`models.md`「给人看的字」）。
        "no_model" => ("没有可用的模型。", "No model is available."),
        "cooling" => (
            "模型冷却中，请稍后重试。",
            "Models cooling down. Try again later.",
        ),
        "model_failed" => ("模型请求失败。", "Model request failed."),
        // 施工 W-8（`web-module.md`「给人看的字」）。
        "bad_code" => ("一次性码已失效。", "One-time code expired."),
        "not_permitted" => ("没有权限。", "Not permitted."),
        "bad_ticket" => ("交接票已失效。", "Ticket expired."),
        "bad_login" => ("登录已失效，请重新登录。", "Login expired. Sign in again."),
        "bad_password" => ("访问密码错误。", "Incorrect access password."),
        "password_not_set" => (
            "还没有设访问密码，请在这台电脑上打开后设置。",
            "No access password yet. Set one on this computer first.",
        ),
        "login_throttled" => (
            "尝试次数过多，请一分钟后重试。",
            "Too many attempts. Try again in a minute.",
        ),
        "setup_first" => (
            "请先设置用户名和密码。",
            "Set a username and password first.",
        ),
        "local_only" => ("仅限本机。", "Local only."),
        "local_machine_only" => (
            "只能在核心所在的机器上用。",
            "Only available on the machine running the core.",
        ),
        _ => ("已拒绝。", "Refused."),
    }
}
