//! 工具说明的同名覆盖（施工 OV-1 上，`miyu_policy::overrides`）：主人家目录 `home/<账号>/tools/<工具>.toml`（施工 SU-3 下：系统区
//! 那一层拿掉）；写坏了的整份不认，`check` 报。会话拼工具面时照它换（`PresetPlaces::overrides`）。
//!
//! 提示词的同名覆盖（施工 OV-1 中，[`resources`]）：`home/<账号>/prompts/<同样的路径>` 压着出厂的，查法在
//! `miyu_store::resources`。造会话、载入会话时照属主交给会话。

use std::collections::BTreeMap;
use std::path::PathBuf;

use miyu_kernel::id::AccountId;
use miyu_policy::overrides::ToolOverride;
use miyu_store::overrides::{TOOLS, tools_in};
use miyu_store::resources::{PROMPTS, ResourceRoot};

use crate::Core;

/// 读好的：工具名到那一份，和写坏了的文件（路径、为什么）。
#[derive(Debug, Default)]
pub(crate) struct Layered {
    pub(crate) merged: BTreeMap<String, ToolOverride>,
    pub(crate) bad: Vec<(PathBuf, String)>,
}

/// 属主 `owner` 家目录里的工具说明覆盖。
pub(crate) fn of(core: &Core, owner: &AccountId) -> Layered {
    let mut layered = Layered::default();
    for written in tools_in(&core.root.account_dir(owner).join(TOOLS)) {
        match written.text.and_then(|text| ToolOverride::parse(&text)) {
            Ok(read) => {
                layered.merged.insert(written.tool, read);
            }
            Err(why) => layered.bad.push((written.path, why)),
        }
    }
    layered
}

/// 写坏了的覆盖（`check` 报的）：读不成、写法不对的，和换不进这个人工具目录里那件工具的（参数格式里没有那个参数），后者照工具名
/// 报写了它的文件。
pub(crate) fn problems(core: &Core, owner: &AccountId) -> Vec<(PathBuf, String)> {
    let Layered { merged, mut bad } = of(core, owner);
    let catalog = core.tools.current();
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
            let path = core
                .root
                .account_dir(owner)
                .join(TOOLS)
                .join(format!("{tool}.toml"));
            bad.push((path, why));
        }
    }
    bad
}

/// 属主 `owner` 的资源目录：压上他家目录里提示词的覆盖（施工 OV-1 中）。
pub(crate) fn resources(core: &Core, owner: &AccountId) -> ResourceRoot {
    core.resources
        .overlaid(vec![core.root.account_dir(owner).join(PROMPTS)])
}
