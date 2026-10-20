//! 规格（`docs/blueprint/sandbox.md`「对外的样子」）：哪些能写，哪些藏起来（读写都不行）；别的都能读，不能写。路径
//! 都是真实的位置。写成 JSON 经命令行交给助手，里面只有路径，没有密钥。照 DeepSeek 的 dsh：整盘能读，只管写；
//! 沙盒只管读写权限，不管网络（2026-09-29 项目主人定）。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 一条命令关进沙盒的规格。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    /// 能写的目录、文件。一条都没有就是全盘只读（`/dev/null` 总能写）。
    #[serde(default)]
    pub write: Vec<PathBuf>,
    /// 读写都不行的，例如数据根：里面有本机令牌，读到就能冒充本人。
    #[serde(default)]
    pub hidden: Vec<PathBuf>,
    /// 只准读这些（施工 5-12 下，外部身份的会话）：写了的，除了它们和 `write` 的，别处都读不到，`hidden` 不再另算；没写的
    /// 照旧整盘能读。这台机器上关不住读的（[`crate::CONFINES_READS`] 是假的），助手收到写了它的规格不跑。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read: Option<Vec<PathBuf>>,
    /// 藏起来的里面单独放开只读的（施工 X-10 中，开着的技能文件夹）：读得到，写不了，`hidden` 照旧挡住它旁边的。没有的不写，
    /// 规格的字节和以前一样。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reveal: Vec<PathBuf>,
    /// 用挂载收紧（施工 5-16，Linux 上有能用的 bubblewrap 时执行器写上）：能写的里面藏着的用挂载盖住，连名字都看不到，路上
    /// 那几层也不冻住。没写的照旧只用 Landlock，规格的字节和以前一样。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mount: bool,
    /// 这条命令要连的 Unix 套接字（施工 M-3，脚本调工具的通道，设计 `39-脚本调工具.md` 第五节第 5 条）：别的套接字照旧连不上，
    /// 这几个放行，只读一级也放行。没有的不写，规格的字节和以前一样。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sockets: Vec<PathBuf>,
}

impl Spec {
    /// 写成一行 JSON：助手的 `--spec` 收的就是它。
    ///
    /// # Errors
    ///
    /// 实际不会出错：几格都是路径。路径不是 UTF-8 的，serde 交回错误。
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 从 JSON 读回来。
    ///
    /// # Errors
    ///
    /// 不是 JSON、有认不得的格、类型不对。
    pub fn from_json(text: &str) -> Result<Spec, serde_json::Error> {
        serde_json::from_str(text)
    }
}

#[cfg(test)]
mod tests;
