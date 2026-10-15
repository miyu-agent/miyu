//! 要先加载的工具用的字（施工 L-1，`docs/designs/25-工具加载.md` W9）：`core/loading/` 下，`load_tools` 的说明和参数、答它的
//! 几句。造会话时冻结进快照的 `core.loading`。

use std::io;

use miyu_kernel::raw::RawJson;
use miyu_policy::LoadingTexts;
use serde::Deserialize;

use super::{ResourceRoot, SourceError};

/// `load_tools.json` 的样子：同工具的说明文件。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecFile {
    description: String,
    parameters: RawJson,
}

impl ResourceRoot {
    /// 要先加载的工具用的字。
    ///
    /// # Errors
    ///
    /// 读不了哪一份；`load_tools.json` 不是 `{description, parameters}` 的样子。
    pub fn loading(&self) -> Result<LoadingTexts, SourceError> {
        let parts = ["core", "loading", "load_tools.json"];
        let spec: SpecFile =
            serde_json::from_str(&self.read(&parts)?).map_err(|error| SourceError::Read {
                path: self.path.join(parts.join("/")),
                error: io::Error::new(io::ErrorKind::InvalidData, error.to_string()),
            })?;
        Ok(LoadingTexts {
            description: spec.description,
            parameters: spec.parameters,
            unknown: self.read(&["core", "loading", "unknown.txt"])?,
            bad_args: self.read(&["core", "loading", "bad-args.txt"])?,
            contract: self.read(&["core", "loading", "contract.txt"])?,
        })
    }
}
