//! 一步工具写成 ACP 的 `tool_call`、`tool_call_update`（设计 73 第五节第 3 条，`internal/blueprint/cli/acp.md`「工具步骤」）：
//! 编号用条目的编号（还在写参数时就有，调用编号要等回复落盘），标题用工具卡片给人看的那一句，`kind` 照工具，读写文件的带
//! `locations`，改了文件的带对比（`diff`，照 `view.detail` 的统一格式拆成一段段改前、改后）。

use std::path::Path;

use serde_json::{Value, json};

use miyu_view::{Tool, ToolState};

/// 工具归到规范的哪一类（编辑器照它挑图标）：认不得的是 `other`。供应商侧的搜索、抓网页照核心认的那几个名字。
pub(crate) fn kind(name: &str) -> &'static str {
    match name {
        "read" => "read",
        "edit" | "write" => "edit",
        "trash" => "delete",
        "glob" | "grep" | "web_search" | "search_web" | "WebSearch" => "search",
        "shell" => "execute",
        "web_fetch" | "read_url_content" | "WebFetch" => "fetch",
        "todowrite" => "think",
        _ => "other",
    }
}

/// 一步的状态：还在写参数、等人确认的 `pending`，在跑的 `in_progress`，成了 `completed`；出错、被拒、打断、跳过的规范里只有
/// `failed` 一种。
pub(crate) fn status(tool: &Tool) -> &'static str {
    match tool.state {
        ToolState::Preparing => "pending",
        ToolState::Running if waiting_for_person(tool) => "pending",
        ToolState::Running => "in_progress",
        ToolState::Ok => "completed",
        _ => "failed",
    }
}

/// 这一步在等人确认：有确认、还没定。
fn waiting_for_person(tool: &Tool) -> bool {
    tool.approval
        .as_ref()
        .is_some_and(|approval| approval.decision.is_none())
}

/// 标题：工具卡片上的那一句，显示名接对象；经谁做的写在名字后面的括号里；有结果那一句的接在 ` · ` 后面（同 `miyu ask` 的一行）。
pub(crate) fn title(tool: &Tool) -> String {
    let title = &tool.title;
    let mut line = match &title.via {
        Some(via) => format!("{} ({via})", title.name),
        None => title.name.clone(),
    };
    if let Some(object) = title.object.as_deref().filter(|object| !object.is_empty()) {
        line.push(' ');
        line.push_str(object);
    }
    if let Some(said) = title.said.as_deref().filter(|said| !said.is_empty()) {
        line.push_str(" · ");
        line.push_str(said);
    }
    line
}

/// 第一次报这一步：`tool_call`。`cwd` 是会话的工作目录，参数里相对的路径照它补全。
pub(crate) fn opened(id: &str, tool: &Tool, cwd: &str) -> Value {
    let mut call = json!({
        "sessionUpdate": "tool_call",
        "toolCallId": id,
        "title": title(tool),
        "name": tool.name,
        "kind": kind(&tool.name),
        "status": status(tool),
    });
    fill(&mut call, tool, cwd);
    call
}

/// 这一步变了：`tool_call_update`，只写变了的；什么都没变的没有。
pub(crate) fn changed(id: &str, before: &Tool, now: &Tool, cwd: &str) -> Option<Value> {
    let mut update = json!({"sessionUpdate": "tool_call_update", "toolCallId": id});
    if title(before) != title(now) {
        update["title"] = json!(title(now));
    }
    if status(before) != status(now) {
        update["status"] = json!(status(now));
    }
    if before.args != now.args
        || before.files != now.files
        || before.output != now.output
        || before.state != now.state
    {
        fill(&mut update, now, cwd);
    }
    (update.as_object().map_or(0, serde_json::Map::len) > 2).then_some(update)
}

/// 补上参数原文、动的文件、结果：参数写完了的才带 `rawInput`；有结果的带结果的字（出错、被拒的是说为什么的那一句）。
fn fill(call: &mut Value, tool: &Tool, cwd: &str) {
    if tool.state != ToolState::Preparing
        && let Ok(input) = serde_json::from_str::<Value>(&tool.args)
    {
        call["rawInput"] = input;
    }
    let places = locations(tool, cwd);
    if !places.is_empty() {
        call["locations"] = json!(places);
    }
    let content = content(tool);
    if !content.is_empty() {
        call["content"] = json!(content);
    }
}

/// 卡片里的内容（`content`，每次整份换）：执行命令的先是命令原文（`$ <命令>`，规范要编辑器把命令写全，确认框里尤其要看得到；
/// 标题只有她写的说明），再是结果的字。参数还没写完的、没有命令也没有结果的是空的。供应商侧工具经驱动照跑命令问过人的，命令照
/// 确认的 `detail`（施工 ACP-12：以前卡片上看不到要跑什么）。
pub(crate) fn content(tool: &Tool) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(command) = command(tool) {
        out.push(text_item(&fenced(&format!("$ {command}"))));
    }
    if let Some(text) = said(tool) {
        out.push(text_item(&text));
    }
    out
}

/// 执行命令的那一行原文：`shell` 参数写完了的 `command`；供应商侧工具是驱动问人时交的 `detail.command`。
fn command(tool: &Tool) -> Option<String> {
    if tool.hosted {
        return asked_command(tool);
    }
    if tool.name != "shell" || tool.state == ToolState::Preparing {
        return None;
    }
    let args = serde_json::from_str::<Value>(&tool.args).ok()?;
    args["command"]
        .as_str()
        .filter(|command| !command.is_empty())
        .map(str::to_string)
}

/// 驱动照跑命令问人时写在确认里的命令（核心的 `driver_ask.rs`：`{driver, tool, title, command}`）。
fn asked_command(tool: &Tool) -> Option<String> {
    let detail = tool.approval.as_ref()?.detail.as_ref()?;
    let detail = serde_json::from_str::<Value>(detail.get()).ok()?;
    detail["command"]
        .as_str()
        .filter(|command| !command.is_empty())
        .map(str::to_string)
}

/// 一项字的内容。
fn text_item(text: &str) -> Value {
    json!({"type": "content", "content": {"type": "text", "text": text}})
}

/// 结果的字：输出原文的开头一段，照代码块写（编辑器照 Markdown 画）；没有输出的、出错被拒的写结果那一句。还没结果的没有。
fn said(tool: &Tool) -> Option<String> {
    if tool.state.busy() {
        return None;
    }
    match tool.output.as_deref().filter(|output| !output.is_empty()) {
        Some(output) => Some(fenced(output)),
        None => tool.title.said.clone().filter(|said| !said.is_empty()),
    }
}

/// 一段字围进代码块：围栏比字里最长的一串反引号多一个，至少三个。
fn fenced(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    let newline = if text.ends_with('\n') { "" } else { "\n" };
    format!("{fence}\n{text}{newline}{fence}")
}

/// 这一步动的文件：结果里改了的那几个（已经是真实位置的绝对路径）；还没结果的，读、写、改文件照参数里的 `file_path`，
/// 相对的照工作目录补全。
fn locations(tool: &Tool, cwd: &str) -> Vec<Value> {
    if !tool.files.is_empty() {
        return tool
            .files
            .iter()
            .map(|file| json!({"path": file.path}))
            .collect();
    }
    if !matches!(tool.name.as_str(), "read" | "edit" | "write") {
        return Vec::new();
    }
    // 照标准写法的（施工 ED-3）：`path` 这些别名也认。
    let Ok(mut args) = serde_json::from_str::<Value>(&tool.args) else {
        return Vec::new();
    };
    miyu_kernel::tool::shape::standard_value(&mut args);
    let Some(written) = args["file_path"].as_str().filter(|path| !path.is_empty()) else {
        return Vec::new();
    };
    let path = Path::new(written);
    let whole = match path.is_absolute() {
        true => path.to_path_buf(),
        false => Path::new(cwd).join(path),
    };
    vec![json!({"path": whole.to_string_lossy()})]
}

/// `view.detail` 回的每个文件的统一格式差异（一行一项）拆成规范的对比：一段 `@@` 一项，改前是上下文加删的行，改后是上下文加加的行；
/// 新建的文件（改前一行都没有）改前写 `null`。算不出差异的（太大、二进制）不报。
pub(crate) fn diffs(detail: &Value) -> Vec<Value> {
    let files = detail["files"].as_array().map_or(&[][..], Vec::as_slice);
    let mut out = Vec::new();
    for file in files {
        // 差异是一行一项的数组（核心现在的写法），也认整段的字。
        let diff = match &file["diff"] {
            Value::Array(lines) => lines
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n"),
            Value::String(text) => text.clone(),
            _ => continue,
        };
        let Some(path) = file["path"].as_str() else {
            continue;
        };
        let created = created(&diff);
        for (old, new) in hunks(&diff) {
            let old = match created && old.is_empty() {
                true => Value::Null,
                false => json!(old),
            };
            out.push(json!({"type": "diff", "path": path, "oldText": old, "newText": new}));
        }
    }
    out
}

/// 是新建的文件：第一段从改前的第 0 行开始。
fn created(diff: &str) -> bool {
    diff.lines()
        .find(|line| line.starts_with("@@"))
        .is_some_and(|line| line.starts_with("@@ -0,0 "))
}

/// 统一格式拆成一段段（改前、改后）。
fn hunks(diff: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in diff.lines() {
        if line.starts_with("@@") {
            out.push((String::new(), String::new()));
            continue;
        }
        let Some((old, new)) = out.last_mut() else {
            continue;
        };
        let mut chars = line.chars();
        let mark = chars.next();
        let rest = chars.as_str();
        match mark {
            Some(' ') => {
                push_line(old, rest);
                push_line(new, rest);
            }
            Some('-') => push_line(old, rest),
            Some('+') => push_line(new, rest),
            _ => {}
        }
    }
    out
}

/// 接一行，带上换行。
fn push_line(text: &mut String, line: &str) {
    text.push_str(line);
    text.push('\n');
}

#[cfg(test)]
mod tests;
