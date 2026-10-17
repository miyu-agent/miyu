//! 卸了的包留下的改动（施工 P-11 下，设计 `31-软件包.md` 第十节第 6 条）：包带的人格、预设卸了就没了，系统区、家目录里同名的
//! 改动留着、照旧能用。这里记一笔是哪个包留下的：`miyu check` 照它提示，重装这个包时它不算撞名。
//!
//! 放在 `<数据根>/state/packages/.leftovers.json`（点开头，和包的状态目录、本地库 `.local` 在同一层撞不上）：
//! `{"personas": {"<编号>": "<包>"}, "presets": {…}}`。只是提示用的，读不了、写坏了的当没有。

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 文件名。
pub const FILE: &str = ".leftovers.json";

/// 留下的人格、预设：编号到留下它的包。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leftovers {
    /// 人格。
    #[serde(default)]
    pub personas: BTreeMap<String, String>,
    /// 预设。
    #[serde(default)]
    pub presets: BTreeMap<String, String>,
}

impl Leftovers {
    /// 读 `state/packages`（`dir`）里的那一份；没有的、读不懂的是空的。
    pub fn read(dir: &Path) -> Leftovers {
        fs::read_to_string(path(dir))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// 写回 `dir`：先写进临时文件再换上；空的删掉文件。
    ///
    /// # Errors
    ///
    /// 建不了目录、写不进、换不上。
    pub fn write(&self, dir: &Path) -> io::Result<()> {
        let path = path(dir);
        if self.personas.is_empty() && self.presets.is_empty() {
            return match fs::remove_file(&path) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            };
        }
        fs::create_dir_all(dir)?;
        let staged = dir.join(format!("{FILE}.new"));
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(&staged, json + "\n")?;
        fs::rename(&staged, &path)
    }

    /// 包 `package` 卸了：它带的人格 `personas`、预设 `presets` 里还有改动留着的记下来（以前记的别的包的盖掉）。
    pub fn left_by(&mut self, package: &str, personas: &[String], presets: &[String]) {
        for id in personas {
            self.personas.insert(id.clone(), package.to_string());
        }
        for id in presets {
            self.presets.insert(id.clone(), package.to_string());
        }
    }

    /// 包 `package` 装回来了：它留下的那几笔划掉。
    pub fn back(&mut self, package: &str) {
        self.personas.retain(|_, by| by != package);
        self.presets.retain(|_, by| by != package);
    }

    /// 人格 `id` 是不是包 `package` 留下的。
    pub fn persona_of(&self, id: &str, package: &str) -> bool {
        self.personas.get(id).is_some_and(|by| by == package)
    }

    /// 预设 `id` 是不是包 `package` 留下的。
    pub fn preset_of(&self, id: &str, package: &str) -> bool {
        self.presets.get(id).is_some_and(|by| by == package)
    }
}

/// `dir` 里那一份的位置。
fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

#[cfg(test)]
mod tests;
