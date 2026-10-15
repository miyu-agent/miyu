//! 要先加载的工具（施工 L-1，`docs/designs/25-工具加载.md` W9）：`load_tools` 由内核当场答（`tool::Loading`）；请求带上它们的完整
//! 参数格式（施工 L-2），驱动给吃不下外壳的那一家编码时换回。

use super::Session;
use crate::block::ToolCall;
use crate::request::Request;
use crate::tool::{Worded, repair};

impl Session {
    /// 这一次调用是 `load_tools` 的（施工 L-1）：交回成没成、写成的一句。参数先照它的参数格式修正（写成字符串的列表照样认）。
    /// 不是的、会话没有要先加载的工具的，没有。
    pub(super) fn loaded(&self, call: &ToolCall) -> Option<(bool, Worded)> {
        let loading = self.policy.loading.as_ref()?;
        if loading.tool != call.name {
            return None;
        }
        let args = match self.policy.tools.get(&call.name) {
            Some(rule) => {
                repair(&rule.parameters, &call.args).unwrap_or_else(|_| call.args.clone())
            }
            None => call.args.clone(),
        };
        Some(loading.answer(&args))
    }

    /// 请求的工具面上要先加载的那几件，完整的参数格式放进 `full_parameters`（施工 L-2）。会话没有要先加载的工具的，原样交回。
    pub(super) fn with_full_parameters(&self, mut request: Request) -> Request {
        let Some(loading) = self.policy.loading.as_ref() else {
            return request;
        };
        let found: Vec<_> = request
            .tools
            .iter()
            .filter_map(|tool| {
                let contract = loading.contracts.get(&tool.name)?;
                Some((tool.name.clone(), contract.parameters.clone()))
            })
            .collect();
        request.full_parameters.extend(found);
        request
    }
}
