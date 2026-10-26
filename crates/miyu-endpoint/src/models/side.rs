//! 扩展登记的旁路请求（施工 Q-3，设计 `43-旁路请求和档位池.md` 第三节第 2、4 条）：清单 `[[side]]` 写的，和核心出厂的
//! （`miyu_models::side::SHIPPED`）一起列进 `model.list` 的 `side`；扩展的 `model.call` 写 `side` 只认自己包里的。只看核心手里
//! 的清单（`Core::system_packages`），不碰磁盘：包卸了，它的登记跟着没了。

use serde_json::{Value, json};

use miyu_config::Values;
use miyu_config::package::SideRequest;
use miyu_models::side::{self, Side, Tier};
use miyu_store::human::Human;

use crate::Core;

/// 一个包登记的一条。
pub(crate) struct Registered {
    /// 登记它的包。
    pub(crate) package: String,
    /// 清单里写的那一段。
    pub(crate) request: SideRequest,
}

impl Registered {
    /// 全编号：`<包>/<编号>`。
    pub(crate) fn id(&self) -> String {
        self.request.full_id(&self.package)
    }
}

/// 装着的扩展登记的全部（清单读成了的）。
pub(crate) fn registered(core: &Core) -> Vec<Registered> {
    core.system_packages()
        .iter()
        .filter_map(|one| Some((one.id.clone(), one.read.as_ref().ok()?)))
        .flat_map(|(package, manifest)| {
            manifest.side.iter().map(move |request| Registered {
                package: package.clone(),
                request: request.clone(),
            })
        })
        .collect()
}

/// 包 `package` 登记的编号 `id` 那一条（`model.call` 的 `side`）；没有的没有。
pub(crate) fn own(core: &Core, package: &str, id: &str) -> Option<SideRequest> {
    registered(core)
        .into_iter()
        .find(|one| one.package == package && one.request.id == id)
        .map(|one| one.request)
}

/// 清单里写的档位换成登记表的（清单只认三个固定池名，认不出的不会有：`miyu-models` 的测试守着两份一样）。
pub(crate) fn tier(request: &SideRequest) -> Tier {
    Tier::parse(request.tier).unwrap_or(Tier::Lite)
}

/// `model.list` 的 `side`（施工 Q-2，Q-3 加扩展的）：每一条的编号、档位、照连接的语言 `language` 的名字和说明、这一刻会落到哪
/// （档位池解析得出的是池名，空的是 `session`：用会话的模型）。出厂的名字、说明在 `core/human` 的 `model/side/<编号>` 下，扩展
/// 的照它清单里写的。照编号排。
pub(crate) fn listed(
    core: &Core,
    values: &Values,
    (human, language): (&Human, &str),
) -> Vec<Value> {
    let resolved = |side: &Side<'_>| side::resolved(values, side).unwrap_or("session");
    let mut all: Vec<Value> = side::SHIPPED
        .iter()
        .map(|side| {
            let said = |key: String| miyu_config::Words::sentence(human, &key, &[]);
            json!({
                "id": side.id,
                "tier": side.tier.pool(),
                "name": said(format!("model/side/{}", side.id)),
                "summary": said(format!("model/side/{}/summary", side.id)),
                "resolved": resolved(side),
            })
        })
        .collect();
    for one in registered(core) {
        let id = one.id();
        let side = Side {
            id: &id,
            tier: tier(&one.request),
        };
        let pick = |phrases| crate::personas::pick(phrases, language).unwrap_or_default();
        all.push(json!({
            "id": id,
            "tier": side.tier.pool(),
            "name": pick(&one.request.name),
            "summary": pick(&one.request.summary),
            "resolved": resolved(&side),
        }));
    }
    all.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    all
}
