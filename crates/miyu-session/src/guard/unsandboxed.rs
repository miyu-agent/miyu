//! 主人放行出沙盒的（施工 X-36，`11-权限与沙盒.md` A15）：回合开始换上这一轮的放行表，判的时候照名字和来处认（`crate::unsandboxed`）。
//! 带文件的工具，文件的真实位置还得落在管得住的地方、又不在她这一轮写得了的地方（施工 X-37 补）。

use std::path::Path;
use std::sync::PoisonError;

use miyu_fs::Zone;
use miyu_kernel::raw::RawJson;

use super::Guard;
use super::asking::{Asked, granted};
use crate::unsandboxed::{Grant, Unsandboxed};

impl Guard {
    /// 换上这一轮主人放行出沙盒的：回合开始照这一轮的配置换，下一次判照新的。
    pub(crate) fn set_unsandboxed(&self, unsandboxed: Unsandboxed) {
        *self
            .unsandboxed
            .write()
            .unwrap_or_else(PoisonError::into_inner) = unsandboxed;
    }

    /// 工具 `name` 主人放行了：照名字和来处认，工具自己报不报都一样。带文件的，文件落在这一次调用（工作目录 `cwd`、加进来的
    /// 目录 `dirs`、放行过的规则 `grants`）写得了的地方的不算（施工 X-37 补，纵深防御：开发时资源目录就是她的工作区）。
    pub(super) fn admin_granted(
        &self,
        name: &str,
        cwd: &str,
        dirs: &[String],
        grants: &[RawJson],
    ) -> bool {
        let grant = self
            .unsandboxed
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .grant(name, &self.catalog);
        match grant {
            Grant::No => false,
            Grant::Whole => true,
            Grant::File(real) => !self.writable(&real, cwd, dirs, grants),
        }
    }

    /// 真实位置 `real` 她这一次写得了：在工作区、加进来的目录、临时目录里（边界表的 [`Zone::Writable`]），或者放行过的写的规则
    /// 管着它（施工 5-15「此目录不再询问」）。
    fn writable(&self, real: &Path, cwd: &str, dirs: &[String], grants: &[RawJson]) -> bool {
        let bounds = self.bounds(cwd, dirs);
        let zone = bounds.boundary.zone(real);
        let asked = Asked::new(real.to_path_buf(), true, zone, &bounds.boundary);
        zone == Zone::Writable || granted(grants, &asked, &bounds.boundary)
    }
}
