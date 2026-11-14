//! 供应商编号到给人看的名字（施工 HT-3，`docs/blueprint/view.md`「标题那一句」）：供应商侧工具那一步的 `via` 写是谁跑的。
//! 先后同 `model.list` 的 `name`（施工 8-21，`models/entry.rs`）：配置里写了的（去掉两头空白不是空的），没写的照目录里对上
//! 的那一家，都没有的不在里面，投影照编号写。

use std::collections::BTreeMap;
use std::sync::Arc;

use miyu_config::Values;
use miyu_models::provider;
use miyu_models::settings::ProviderSettings;
use miyu_session::ModelData;

use crate::Core;

/// 读名字要的：这一刻的配置、模型资料。拿到阻塞线程里再读（查目录可能碰磁盘）。
#[derive(Clone)]
pub(crate) struct Providers {
    values: Values,
    data: Arc<ModelData>,
}

impl Providers {
    /// 这个核心这一刻的。
    pub(crate) fn of(core: &Core) -> Providers {
        Providers {
            values: core.config().resolved().values(),
            data: Arc::clone(&core.model_data),
        }
    }

    /// 配好的每一家：编号到给人看的名字，没有名字的不在里面。可能碰磁盘。
    pub(crate) fn names(&self) -> BTreeMap<String, String> {
        miyu_config::key::names(self.values.keys(), "providers.<id>", &[])
            .into_iter()
            .filter_map(|id| {
                let written = ProviderSettings::at(&self.values, &[&id])
                    .name
                    .map(|name| name.trim().to_string())
                    .filter(|name| !name.is_empty());
                let name = written.or_else(|| {
                    self.data.with(|knowledge| {
                        let found = provider::provider(&self.values, knowledge, &id).ok()?;
                        let recognized = found.recognized?;
                        knowledge
                            .lookup
                            .as_ref()?
                            .listing(&recognized.provider)?
                            .name
                    })
                })?;
                Some((id, name))
            })
            .collect()
    }
}
