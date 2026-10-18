//! 问人之前先看一眼（施工 F-10 下，`docs/blueprint/tools/package.md`）：权限策略要问人时，工具要看一眼的（[`Tool::preview`]，
//! 现在只有 `package`），执行器带着端口调它。成了的整份放进确认请求的说明，键 `preview`：人看到的就是日志里记下的；不成的
//! 不问，照拒绝交给她。不要看的照原样问。

use std::sync::Arc;

use serde_json::Value;

use miyu_kernel::block::Block;
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Verdict;
use miyu_tool::{Call, Stop, Tool};

use super::Tools;

impl Tools {
    /// 权限策略判下来的 `verdict`：要问人的、工具 `name` 要看一眼的，带着端口照参数 `args`、工作目录 `cwd` 看，换进说明；
    /// 看不了的换成拒绝。别的原样交回。
    pub(crate) async fn previewed(
        &self,
        name: &str,
        args: String,
        cwd: String,
        verdict: Verdict,
    ) -> Verdict {
        let Verdict::Ask {
            module,
            access,
            rule,
            detail,
        } = verdict
        else {
            return verdict;
        };
        let looked = match self.catalog.get(name) {
            Some(tool) => self.look(tool, args, cwd).await,
            None => None,
        };
        match looked {
            None => Verdict::Ask {
                module,
                access,
                rule,
                detail,
            },
            Some(Ok(preview)) => Verdict::Ask {
                detail: Some(with_preview(detail, preview)),
                module,
                access,
                rule,
            },
            Some(Err(done)) => Verdict::Deny {
                module,
                text: done
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::Text(text) => Some(text.text.as_str()),
                        _ => None,
                    })
                    .collect(),
                human: done.human,
            },
        }
    }

    /// 带着端口看一眼：只给看要用的那几样，没有沙盒、日志、调用编号。
    async fn look(
        &self,
        tool: Arc<dyn Tool>,
        args: String,
        cwd: String,
    ) -> Option<Result<Value, miyu_tool::Done>> {
        let call = Call {
            args,
            cwd,
            home: self.home.clone(),
            data_root: Some(self.data_root.clone()),
            seen: Arc::default(),
            stop: Stop::default(),
            sandbox: None,
            log: None,
            offset: self.offset,
            agents: None,
            messages: None,
            jobs: None,
            sessions: None,
            usage: None,
            questions: None,
            memory: None,
            knowledge: None,
            packages: self.packages.clone(),
            goals: None,
            ids: None,
            settings: None,
        };
        Some(tool.preview(call)?.await)
    }
}

/// 说明里加一格 `preview`：原来没有说明的、不是对象的，只放这一格。
fn with_preview(detail: Option<RawJson>, preview: Value) -> RawJson {
    let mut detail = detail
        .and_then(|raw| serde_json::from_str::<Value>(raw.get()).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    detail["preview"] = preview;
    serde_json::from_str(&detail.to_string()).expect("自己拼的 JSON 读得回来")
}

#[cfg(test)]
mod tests;
