//! 包带的人格、预设（施工 P-11 下，设计 `31-软件包.md` 第十节第 4、6 条）：装之前和几层里有的比；卸了以后家目录里
//! 同名的改动留着，记一笔是哪个包留下的（`miyu_store::packages::leftovers`），重装时不算撞，`miyu check` 提示。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_store::human::Human;
use miyu_store::layers::Layer;
use miyu_store::packages::conflicts::Kind;
use miyu_store::packages::leftovers::Leftovers;

use super::packages;
use crate::Core;

/// 装包 `id` 之前比的几层（`conflicts` 的 `layers`）：人格、预设的编号在出厂、家目录里有的那几层。不算的：这个包以前
/// 卸掉时留下的改动（重装它，它接着叠在下面）；升级时装着的这一份已经带着的（同 F-8 六补「升级不和自己比」，改动就是改它的）。
/// 碰磁盘，调的一方放在阻塞线程里调它交回的那一个。
pub(super) fn layers(core: &Core, id: &str) -> impl Fn(Kind, &str) -> Vec<Layer> + Send + 'static {
    let (personas, presets) = (
        crate::personas::personas(core),
        crate::presets::presets(core),
    );
    let state = packages(core).state_root().to_path_buf();
    let mine = core
        .system_packages()
        .iter()
        .find(|one| one.id == id)
        .map(|one| one.files_dir());
    let id = id.to_string();
    move |kind, name| {
        let left = Leftovers::read(&state);
        let has = |carried: fn(&Path) -> Vec<String>| {
            mine.as_deref()
                .is_some_and(|dir| carried(dir).iter().any(|one| one == name))
        };
        match kind {
            Kind::Persona if !left.persona_of(name, &id) && !has(miyu_store::personas::carried) => {
                personas.layers_with(name)
            }
            Kind::Preset if !left.preset_of(name, &id) && !has(miyu_store::presets::carried) => {
                presets.layers_with(name)
            }
            _ => Vec::new(),
        }
    }
}

/// 卸包 `id`（文件夹 `dir`）时要记的：它带的人格、预设里家目录还有改动的。交回的那一个碰磁盘，在包目录删掉之前调，
/// 卸成了再 [`Left::keep`]。
pub(super) fn noting(core: &Core, id: &str, dir: &Path) -> impl FnOnce() -> Left + Send + 'static {
    let (personas, presets) = (
        crate::personas::personas(core),
        crate::presets::presets(core),
    );
    let state = packages(core).state_root().to_path_buf();
    let (id, dir) = (id.to_string(), dir.to_path_buf());
    move || {
        let edited = |layers: Vec<Layer>| layers.iter().any(|layer| matches!(layer, Layer::Home));
        let personas = miyu_store::personas::carried(&dir)
            .into_iter()
            .filter(|one| edited(personas.layers_with(one)))
            .collect();
        let presets = miyu_store::presets::carried(&dir)
            .into_iter()
            .filter(|one| edited(presets.layers_with(one)))
            .collect();
        Left {
            state,
            id,
            personas,
            presets,
        }
    }
}

/// 卸包时要记下的留下的改动（[`noting`]）。
pub(super) struct Left {
    state: PathBuf,
    id: String,
    personas: Vec<String>,
    presets: Vec<String>,
}

impl Left {
    /// 卸成了：记下来。没有要记的什么都不写。碰磁盘。
    pub(super) fn keep(self) {
        if self.personas.is_empty() && self.presets.is_empty() {
            return;
        }
        let mut left = Leftovers::read(&self.state);
        left.left_by(&self.id, &self.personas, &self.presets);
        if let Err(error) = left.write(&self.state) {
            tracing::warn!(target: super::TARGET, package = self.id.as_str(), error = %error, "package leftovers not noted");
        }
    }
}

/// 包 `id` 装上了：它以前留下的那几笔划掉。碰磁盘。
pub(super) fn back(core: &Core, id: &str) -> impl FnOnce() + Send + 'static {
    let state = packages(core).state_root().to_path_buf();
    let id = id.to_string();
    move || {
        let mut left = Leftovers::read(&state);
        let before = left.clone();
        left.back(&id);
        if left != before
            && let Err(error) = left.write(&state)
        {
            tracing::warn!(target: super::TARGET, package = id.as_str(), error = %error, "package leftovers not cleared");
        }
    }
}

/// `miyu check` 的提示（施工 P-11 下）：记着的、包没装着、家目录里还有的人格、预设，一条一个，`persona_left`、
/// `preset_left`，`file` 是那一份改动在哪（主人家目录里）。
pub(crate) async fn warnings(core: &Core, words: &Human) -> Vec<Value> {
    let (personas, presets) = (
        crate::personas::personas(core),
        crate::presets::presets(core),
    );
    let state = packages(core).state_root().to_path_buf();
    let installed: Vec<String> = core
        .system_packages()
        .iter()
        .map(|one| one.id.clone())
        .collect();
    let dirs = [(Layer::Home, core.root.path().to_path_buf())];
    let found = tokio::task::spawn_blocking(move || {
        let left = Leftovers::read(&state);
        // 一笔：哪一样、编号、包、那一份改动在哪；包又装着了的、改动删掉了的不算。
        let edited = |layers: Vec<Layer>, sub: &str, name: &str| {
            dirs.iter()
                .find(|(layer, _)| layers.contains(layer))
                .map(|(_, dir)| dir.join(sub).join(name))
        };
        let mut out = Vec::new();
        for (id, package) in left
            .personas
            .iter()
            .filter(|(_, by)| !installed.contains(by))
        {
            if let Some(file) = edited(personas.layers_with(id), "personas", id) {
                out.push(("persona", id.clone(), package.clone(), file));
            }
        }
        for (id, package) in left
            .presets
            .iter()
            .filter(|(_, by)| !installed.contains(by))
        {
            if let Some(file) = edited(presets.layers_with(id), "presets", &format!("{id}.toml")) {
                out.push(("preset", id.clone(), package.clone(), file));
            }
        }
        out
    })
    .await
    .unwrap_or_default();
    found
        .into_iter()
        .map(|(kind, _, package, file)| {
            let code = format!("{kind}_left");
            let message = super::sentence(words, &code, &package).unwrap_or_default();
            json!({"kind": kind, "file": crate::check::shown(core, &file), "code": code, "level": "warning", "message": message})
        })
        .collect()
}
