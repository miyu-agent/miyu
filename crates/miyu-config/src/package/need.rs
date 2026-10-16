//! 包要这个地方有什么才有用（施工 X-25，`docs/blueprint/packages.md`「清单」）：清单 `[package] needs` 写的。现在只认
//! `images`：整包都要「这个地方显示得了图片」，显示不了图片的会话里这个包当没开（`venues.md`「`venue.session`」的
//! `images`）。

/// 一样要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Need {
    /// 这个地方显示得了图片。
    Images,
}

impl Need {
    /// 清单上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Need::Images => "images",
        }
    }

    /// 照写法认；不认识的没有。
    pub fn parse(text: &str) -> Option<Need> {
        match text {
            "images" => Some(Need::Images),
            _ => None,
        }
    }
}
