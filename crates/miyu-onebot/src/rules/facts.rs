//! 给她看的事实的模板（施工 O-25 下，`onebot.md` 第一条「退信」第 3 条、「场所规则和出厂数据」第 1、2 条；O-36 多「另一条线正在
//! 回」，「群里怎么叫她」第 7 条）：包文件夹的 `data/facts/` 里的两份（给模型看的字，登记在 `26-提示词.md` 第十节），桥起来
//! 时和别的出厂数据一起读，交内核的 [`Template::parse`] 查。没有系统那一份：给模型看的字随包走。
//!
//! 字段只认各自的那几个：多要了别的，填的时候换不出来，起来时就当打包的错（「施工时定的」第 129 条）。少要几个照收。

use std::path::Path;

use miyu_chat::{Problem, Source};
use miyu_client::protocol::Template;
use miyu_client::settings::problem::Code;

use super::{files, required};

/// 退信的模板在出厂目录里的位置；问题里的文件名也照它写。
const UNDELIVERED: &str = "facts/undelivered.txt";

/// 退信的模板认的字段：为什么、NapCat 说的原因、她那一段的开头（「退信」第 3 条）。
const UNDELIVERED_FIELDS: &[&str] = &["why", "detail", "text"];

/// 「另一条线正在回」的模板在出厂目录里的位置（施工 O-36）。
const ANOTHER_LINE: &str = "facts/another-line.txt";

/// 「另一条线正在回」认的字段：别的线在回的那一条、这条线回的那一条的平台编号（「群里怎么叫她」第 7 条「事实」）。
const ANOTHER_LINE_FIELDS: &[&str] = &["msg", "aim"];

/// 查过的几份模板。
#[derive(Debug)]
pub(super) struct Facts {
    /// 退信（施工 O-25 下）。
    pub(super) undelivered: Template,
    /// 另一条线正在回（施工 O-36）。
    pub(super) another_line: Template,
}

/// 读出厂目录 `dir`（包文件夹的 `data/`）里的几份模板，查过交回。问题照退信、另一条线的先后记；有问题的交回空的。
pub(super) fn load(dir: &Path, problems: &mut Vec<Problem>) -> Option<Facts> {
    let undelivered = template(dir, UNDELIVERED, UNDELIVERED_FIELDS, problems);
    let another_line = template(dir, ANOTHER_LINE, ANOTHER_LINE_FIELDS, problems);
    Some(Facts {
        undelivered: undelivered?,
        another_line: another_line?,
    })
}

/// 读出厂目录 `dir` 里的模板 `name`，查过交回。不在、读不成的记一条问题；写坏了、要了 `fields` 以外的字段的记一条 `bad_format`，
/// 原话说错在哪。有问题的交回空的。
fn template(
    dir: &Path,
    name: &str,
    fields: &[&str],
    problems: &mut Vec<Problem>,
) -> Option<Template> {
    let text = required(&dir.join(name), name, problems)?;
    let checked = Template::parse(&text)
        .map_err(|error| error.to_string())
        .and_then(|template| {
            match template
                .fields()
                .iter()
                .find(|field| !fields.contains(field))
            {
                Some(field) => Err(format!("unknown field {{{field}}}: only {fields:?}")),
                None => Ok(template),
            }
        });
    match checked {
        Ok(template) => Some(template),
        Err(why) => {
            problems.push(files::whole(
                Code::BadFormat,
                Source::Factory,
                name,
                Some(why),
            ));
            None
        }
    }
}
