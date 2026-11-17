//! 一家供应商的驱动从哪来（施工 8-26）：`model.list` 的 `driver_from`。施工 GO-1 从 `provider.rs` 挪出来（原文件到了行数上限）。

/// 一家供应商的驱动从哪来（施工 8-26）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverFrom {
    /// 配置里手写的。
    Config,
    /// 档案（`profiles.toml`）写的。
    Profile,
    /// 照目录里那一家的 `npm` 换的。
    Catalog,
    /// 都没有，用 [`super::DEFAULT_DRIVER`]。
    Default,
}

impl DriverFrom {
    /// `model.list` 里的写法：`config`、`profile`、`catalog`、`default`。
    pub fn as_str(self) -> &'static str {
        match self {
            DriverFrom::Config => "config",
            DriverFrom::Profile => "profile",
            DriverFrom::Catalog => "catalog",
            DriverFrom::Default => "default",
        }
    }
}
