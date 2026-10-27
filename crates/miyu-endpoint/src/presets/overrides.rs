//! 工具说明的同名覆盖（施工 OV-1 上，`miyu_policy::overrides`）：照属主找两层，系统区 `system/tools/<工具>.toml`、他家目录
//! `home/<账号>/tools/<工具>.toml`，他的压着系统的、一格一格压；写坏了的整份不认，照下一层，`check` 报。会话拼工具面时照它换
//! （`PresetPlaces::overrides`）。
//!
//! 提示词的同名覆盖（施工 OV-1 中，[`resources`]）：`home/<账号>/prompts/<同样的路径>` 压着 `system/prompts/<同样的路径>` 压着
//! 出厂的，查法在 `miyu_store::resources`。造会话、载入会话时照属主交给会话。

use std::collections::BTreeMap;
use std::path::PathBuf;

use miyu_kernel::id::AccountId;
use miyu_policy::overrides::ToolOverride;
use miyu_store::overrides::{TOOLS, tools_in};
use miyu_store::resources::{PROMPTS, ResourceRoot};

use crate::Core;

/// 照属主叠好的：工具名到那一份，和写坏了的文件（路径、为什么）。
#[derive(Debug, Default)]
pub(crate) struct Layered {
    pub(crate) merged: BTreeMap<String, ToolOverride>,
    pub(crate) bad: Vec<(PathBuf, String)>,
}

/// 属主 `owner` 的工具说明覆盖：系统区那一层先读，他自己那一层压在上面。
pub(crate) fn of(core: &Core, owner: &AccountId) -> Layered {
    let layers = [
        core.root.system().join(TOOLS),
        core.root.account_dir(owner).join(TOOLS),
    ];
    let mut layered = Layered::default();
    for dir in layers {
        for written in tools_in(&dir) {
            match written.text.and_then(|text| ToolOverride::parse(&text)) {
                Ok(read) => {
                    let lower = layered.merged.remove(&written.tool).unwrap_or_default();
                    layered.merged.insert(written.tool, read.over(lower));
                }
                Err(why) => layered.bad.push((written.path, why)),
            }
        }
    }
    layered
}

/// 写坏了的覆盖（`check` 报的）：读不成、写法不对的，和叠好以后换不进这个人工具目录里那件工具的（参数格式里没有那个参数），
/// 后者照工具名报他那一层或者系统那一层里写了它的文件。
pub(crate) fn problems(core: &Core, owner: &AccountId) -> Vec<(PathBuf, String)> {
    let Layered { merged, mut bad } = of(core, owner);
    let catalog = core.catalog_of(owner);
    for (tool, written) in merged {
        let Some(spec) = catalog.get(&tool).map(|found| found.spec().clone()) else {
            continue;
        };
        let mut entry = miyu_policy::ToolEntry {
            name: spec.name,
            description: spec.description,
            parameters: spec.parameters,
            access: spec.access,
            clicall: false,
            runs_commands: false,
            command_only: false,
        };
        if let Err(why) = written.apply(&mut entry) {
            let file = format!("{tool}.toml");
            let path = [core.root.account_dir(owner), core.root.system()]
                .into_iter()
                .map(|layer| layer.join(TOOLS).join(&file))
                .find(|path| path.is_file())
                .unwrap_or_else(|| core.root.system().join(TOOLS).join(&file));
            bad.push((path, why));
        }
    }
    bad
}

/// 属主 `owner` 的资源目录：压上他自己、系统区两层提示词的覆盖（施工 OV-1 中）。
pub(crate) fn resources(core: &Core, owner: &AccountId) -> ResourceRoot {
    core.resources.overlaid(vec![
        core.root.account_dir(owner).join(PROMPTS),
        core.root.system().join(PROMPTS),
    ])
}
