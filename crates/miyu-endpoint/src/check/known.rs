//! `check` 查配置里选人格、选知识库、选预设的项（施工 PC-3，设计 53 第四节；预设是 PC-3 补）要的名单：现有的人格、知识库、预设的编号（从 `check.rs` 分出来，
//! 行数）。

use crate::Core;
use crate::config::checking::Known;
use crate::personas::personas;
use crate::presets::presets;

/// 现有的人格、知识库、预设的编号（施工 PC-3、PC-3 补）：配置里写的选人格、选知识库、选预设的项没有的报警告。要读盘，放进阻塞
/// 线程；列不出来的那一种不查。
pub(super) async fn known(core: &Core) -> Known {
    let personas = personas(core);
    let presets = presets(core);
    let knowledge = std::sync::Arc::clone(&core.knowledge);
    tokio::task::spawn_blocking(move || Known {
        personas: Some(personas.ids().into_iter().collect()),
        knowledge: knowledge
            .list()
            .ok()
            .map(|(bases, _)| bases.into_iter().map(|base| base.id).collect()),
        presets: Some(presets.ids().into_iter().collect()),
    })
    .await
    .unwrap_or_default()
}
