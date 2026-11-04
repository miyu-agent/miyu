//! 开头（调研第五节第 2 条 `read` 的「开头」）：标题、时间段、模型、token 合计。

use crate::entry::Body;

use super::text::{Out, one_line};
use super::{Document, Say};

/// 写开头：一级标题，下面一个列表。模型照每一轮收尾记的、没撤掉的，去重照先后；一轮都没调成的不写这一行。
pub(super) fn write(out: &mut Out, doc: &Document<'_>, say: &Say<'_>) {
    let about = doc.about;
    let title = one_line(&about.title);
    let title = match title.is_empty() {
        true => say.say("untitled", &[]),
        false => title,
    };
    out.block(&format!("# {title}"));
    let mut lines = vec![say.say("head/time", &[("range", range(doc))])];
    let mut models: Vec<String> = Vec::new();
    for entry in doc.entries.iter().filter(|entry| !entry.hidden) {
        if let Body::End(end) = &entry.body
            && let Some(model) = &end.model
        {
            let model = match &end.endpoint {
                Some(endpoint) => format!("{endpoint}/{model}"),
                None => model.to_string(),
            };
            if !models.contains(&model) {
                models.push(model);
            }
        }
    }
    if !models.is_empty() {
        lines.push(say.say("head/model", &[("models", models.join(", "))]));
    }
    lines.push(say.say("head/tokens", &[("count", about.tokens.to_string())]));
    let list: Vec<String> = lines.iter().map(|line| format!("- {line}")).collect();
    out.block(&list.join("\n"));
}

/// 时间段：照时区写到分钟，同一天的后一头只写钟点，末尾带时区。
fn range(doc: &Document<'_>) -> String {
    let (from, to, offset) = (doc.about.from, doc.about.to, doc.offset);
    let end = match from.local_date(offset) == to.local_date(offset) {
        true => to.local_clock(offset),
        false => to.local_minute(offset),
    };
    format!("{} – {end} ({offset})", from.local_minute(offset))
}
