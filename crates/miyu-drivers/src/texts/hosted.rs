//! 供应商侧工具写成的那一段字（施工 HT-2，`docs/designs/71-供应商侧工具.md` 第三节第 5 条，
//! `docs/blueprint/drivers/openai-chat.md` 第 4 条）：模型在供应商那边自己跑了一次工具（搜索、读网页），日志里记成一块
//! `hosted_tool`（`kernel/blocks.md`）。发请求时三个内置驱动都把它写成一段字，放回它在 assistant 消息里原来的位置，让
//! 这一次的模型知道上一个模型搜过什么、看到了什么；交给扩展驱动的请求里，别家的也换成同一段字（`miyu-session` 的
//! `route/extension/wire.rs`）。
//!
//! 四份模板：有结果的、结果是失败的、没结果的（被打断、供应商没报），和结果截过的那一句。`name` 在属性里，照模板的规矩
//! 转义；参数、结果的字原样放进外壳（JSON 的引号、多行结果的换行都照原样，模型才读得顺），只挡收尾标签（[`guard`]）。
//! 结果的字在这里先截到 [`HOSTED_TEXT_BYTES`]（原文的字节数，字的边界）：日志里留 64 KiB 给人看，进上下文的只要够模型
//! 知道搜过什么。块里 `cut` 是真的，或者这里又截了的，结果的字后面另起一行接截过的那一句，仍在 `<result>`/`<error>` 里面。
//! 交回的那段字不带模板末尾的换行：前后怎么分开由驱动定（Anthropic 是单独一块，这一家不收以空白结尾的最后一条
//! assistant；正文是一整个字符串的两家前后补换行）。
//!
//! 以前造的快照里没有这四份：交回没有，驱动不写它（同 HT-1 的行为），不多出没登记的字。

use std::collections::BTreeMap;

use miyu_kernel::block::HostedTool;
use miyu_kernel::template::{Template, TemplateError, escape};

use super::{DriverTexts, render};

/// 供应商侧工具的结果进请求时至多多少个字节：4 KiB，截在字的边界上（施工 HT-2）。
pub const HOSTED_TEXT_BYTES: usize = 4 * 1024;

/// 供应商侧工具的四份模板（施工 HT-2）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Hosted {
    /// 有结果的：字段是 `name`、`args`、`text`。
    done: Template,
    /// 结果是失败的：字段同上。
    failed: Template,
    /// 没结果的：字段是 `name`、`args`。
    open: Template,
    /// 结果截过的那一句：没有字段。
    cut: Template,
}

/// 供应商侧工具的四份模板的原文（施工 HT-2）。
#[derive(Debug, Clone, Copy)]
pub struct HostedToolSources<'a> {
    /// `hosted-tool.txt`。
    pub hosted_tool: &'a str,
    /// `hosted-tool-failed.txt`。
    pub hosted_tool_failed: &'a str,
    /// `hosted-tool-open.txt`。
    pub hosted_tool_open: &'a str,
    /// `hosted-tool-cut.txt`。
    pub hosted_tool_cut: &'a str,
}

impl Hosted {
    /// 读四份模板，拿字段试着换一次：没结果的那一份只能要 `name`、`args`，截过的那一句不能要字段。
    pub(super) fn new(sources: HostedToolSources<'_>) -> Result<Hosted, TemplateError> {
        let texts = Hosted {
            done: Template::parse(sources.hosted_tool)?,
            failed: Template::parse(sources.hosted_tool_failed)?,
            open: Template::parse(sources.hosted_tool_open)?,
            cut: Template::parse(sources.hosted_tool_cut)?,
        };
        texts.done.render(&fields("", "", Some("")))?;
        texts.failed.render(&fields("", "", Some("")))?;
        texts.open.render(&fields("", "", None))?;
        texts.cut.render(&BTreeMap::new())?;
        Ok(texts)
    }
}

impl DriverTexts {
    /// 一块供应商侧工具（施工 HT-2）写成的那一段字：有结果的照结果是不是失败的挑模板，没结果的只有外壳和参数；结果的字
    /// 截到 [`HOSTED_TEXT_BYTES`]，截过的（块里记着的、这里又截的）另起一行接那一句。快照里没有这几份的（以前造的）交回
    /// 没有：不写它。
    pub fn hosted_tool(&self, hosted: &HostedTool) -> Option<String> {
        let texts = self.hosted.as_ref()?;
        let (name, args) = (escape(&hosted.name), guard(&hosted.args));
        let Some(outcome) = &hosted.outcome else {
            return Some(filled(&texts.open, &fields(&name, &args, None)));
        };
        let shown = shown(&outcome.text);
        let mut text = guard(shown);
        if outcome.cut || shown.len() < outcome.text.len() {
            text.push('\n');
            text.push_str(render(&texts.cut, &BTreeMap::new()).trim_end());
        }
        let template = match outcome.failed {
            true => &texts.failed,
            false => &texts.done,
        };
        Some(filled(template, &fields(&name, &args, Some(&text))))
    }
}

/// 参数、结果的字放进外壳以前只做一件事：每一处 `</` 写成 `<\/`，别的字一个不动。
///
/// 为什么只挡这个：同文本文件、替它看的图的转述一样，外壳里的字原样放，模型读得顺（JSON 的引号、结果的换行不变成
/// `\u0022`、`\n`）；可这里的字是网上来的，多挡一道收尾标签：没有 `</`，它就写不出 `</hosted-tool>`、`</result>`、
/// `</error>`，收不住外壳、跳不出去冒充别的。
fn guard(text: &str) -> String {
    text.replace("</", "<\\/")
}

/// 填进已经处理好的字段（`name` 转义过，参数、结果挡过收尾标签），不再转义；去掉模板末尾的换行，前后怎么分开由驱动定。
fn filled(template: &Template, fields: &BTreeMap<&str, &str>) -> String {
    let mut out = template
        .fill(fields, str::to_string)
        .expect("造的时候试换过，字段都有");
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// 进请求的那一截：不超过 [`HOSTED_TEXT_BYTES`] 的整份；超了的截到它以内、最后一个字的边界上。
fn shown(text: &str) -> &str {
    if text.len() <= HOSTED_TEXT_BYTES {
        return text;
    }
    let mut end = HOSTED_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// 模板的字段：工具名、参数，有结果的另加结果的字。
fn fields<'a>(name: &'a str, args: &'a str, text: Option<&'a str>) -> BTreeMap<&'a str, &'a str> {
    let mut fields = BTreeMap::from([("name", name), ("args", args)]);
    if let Some(text) = text {
        fields.insert("text", text);
    }
    fields
}

#[cfg(test)]
mod tests;
