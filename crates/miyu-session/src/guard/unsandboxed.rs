//! 管理员放行出沙盒的（施工 X-36，`11-权限与沙盒.md` A15）：回合开始换上这一轮的放行表，判的时候照名字和来处认（`crate::unsandboxed`）。

use std::sync::PoisonError;

use super::Guard;
use crate::unsandboxed::Unsandboxed;

impl Guard {
    /// 换上这一轮管理员放行出沙盒的：回合开始照这一轮的配置换，下一次判照新的。
    pub(crate) fn set_unsandboxed(&self, unsandboxed: Unsandboxed) {
        *self
            .unsandboxed
            .write()
            .unwrap_or_else(PoisonError::into_inner) = unsandboxed;
    }

    /// 工具 `name` 管理员放行了：照名字和来处认，工具自己报不报都一样。
    pub(super) fn admin_granted(&self, name: &str) -> bool {
        self.unsandboxed
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .granted(name, &self.catalog)
    }
}
