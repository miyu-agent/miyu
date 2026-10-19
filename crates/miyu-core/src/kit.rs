//! 内置包里有自己家底的（施工 N-2 下）：联网包抓网页记着的页、搜网页的冷却和 MCP 会话。一个核心一份：起来时登记的工具、装卸
//! 软件包以后端点重新要的工具（`miyu_endpoint::builtins`）都用这一份，冷却不因为装卸从头算；设置页的下拉照它写「冷却到几点」。

#[cfg(feature = "net")]
use std::sync::Arc;

use miyu_endpoint::builtins::OptionState;
use miyu_store::resources::ResourceRoot;

/// 一个核心的那一份家底。
#[derive(Clone)]
pub struct Kit {
    #[cfg(feature = "net")]
    pages: Arc<miyu_net::Pages>,
    #[cfg(feature = "net")]
    search: Arc<miyu_net::Search>,
}

impl Kit {
    /// 照资源目录 `resources` 备一份：都是懒的，第一次用才读规矩。
    pub fn new(resources: &ResourceRoot) -> Kit {
        #[cfg(not(feature = "net"))]
        let _ = resources;
        Kit {
            #[cfg(feature = "net")]
            pages: Arc::new(miyu_net::Pages::new(resources.path())),
            #[cfg(feature = "net")]
            search: Arc::new(miyu_net::Search::new(resources.path())),
        }
    }

    /// 测试用：搜网页照给的 `search`。
    #[cfg(all(test, feature = "net"))]
    fn with_search(resources: &ResourceRoot, search: Arc<miyu_net::Search>) -> Kit {
        Kit {
            pages: Arc::new(miyu_net::Pages::new(resources.path())),
            search,
        }
    }

    /// 核心起来以后，搜网页能问扩展声明的来源了（施工 N-5）：拿着核心的弱引用。
    #[cfg(feature = "net")]
    pub(crate) fn attach(&self, core: &Arc<miyu_endpoint::Core>) {
        self.search
            .attach(Arc::new(search::Extensions(Arc::downgrade(core))));
    }

    /// 联网包的工具：抓网页、搜网页照这一份家底。
    #[cfg(feature = "net")]
    pub(crate) fn net_tools(
        &self,
        resources: &ResourceRoot,
    ) -> Result<Vec<Arc<dyn miyu_tool::Tool>>, miyu_tool::load::LoadError> {
        miyu_net::tools_with(
            resources.path(),
            Arc::clone(&self.pages),
            Arc::clone(&self.search),
        )
    }

    /// 配置项 `key` 的选项 `option` 这一刻的状态：搜索来源冷却着的那一家写冷却到什么时候，要 key、要地址没配的照最终值
    /// `values` 说（施工 N-3 上）。
    pub(crate) fn option_state(
        &self,
        key: &str,
        option: &str,
        values: &miyu_config::Values,
    ) -> Option<OptionState> {
        #[cfg(feature = "net")]
        if miyu_net::SearchSettings::ITEMS
            .iter()
            .any(|item| item.key == key)
        {
            let cooling = self
                .search
                .cooling()
                .into_iter()
                .find(|(id, _)| id == option)
                .map(|(_, until)| OptionState::CoolingUntil(until));
            return cooling.or_else(|| match miyu_net::missing(option, values)? {
                miyu_net::Missing::Key => Some(OptionState::NeedsKey),
                miyu_net::Missing::Address => Some(OptionState::NeedsAddress),
            });
        }
        #[cfg(not(feature = "net"))]
        let _ = (key, option, values);
        None
    }
}

#[cfg(feature = "net")]
mod search;
#[cfg(all(test, feature = "net"))]
mod tests;
