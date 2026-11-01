//! `check` 查配置里选人格、选知识库的项（施工 PC-3，设计 53 第四节）要的名单：现有的人格、知识库的编号（从 `check.rs` 分出来，
//! 行数）。

use crate::Core;
use crate::config::checking::Known;
use crate::personas::personas;

/// 现有的人格、知识库的编号（施工 PC-3）：配置里写的选人格、选知识库的项没有的报警告。知识库要读盘，放进阻塞线程；列不出来的
/// 那一种不查。
pub(super) async fn known(core: &Core) -> Known {
    let personas = personas(core);
    let knowledge = std::sync::Arc::clone(&core.knowledge);
    tokio::task::spawn_blocking(move || {
        let personas = Some(personas.ids().into_iter().collect());
        let knowledge = knowledge
            .list()
            .ok()
            .map(|(bases, _)| bases.into_iter().map(|base| base.id).collect());
        (personas, knowledge)
    })
    .await
    .map(|(personas, knowledge)| Known {
        personas,
        knowledge,
    })
    .unwrap_or_default()
}
