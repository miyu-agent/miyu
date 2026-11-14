//! 包要这个地方有什么才有用（施工 X-25，`docs/blueprint/packages.md`「清单」）：清单 `[package] needs` 写的。认两个词：
//! `images`：整包都要「这个地方显示得了图片」，显示不了图片的会话里这个包当没开（`venues.md`「`venue.session`」的
//! `images`）；`life`（施工 LF-4 下）：只在没结束的人生会话里有，别的会话里整包当没开（`life.md`「结束」）。

/// 一样要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Need {
    /// 这个地方显示得了图片。
    Images,
    /// 这是没结束的人生会话（施工 LF-4 下）。
    Life,
}

impl Need {
    /// 清单上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Need::Images => "images",
            Need::Life => "life",
        }
    }

    /// 照写法认；不认识的没有。
    pub fn parse(text: &str) -> Option<Need> {
        match text {
            "images" => Some(Need::Images),
            "life" => Some(Need::Life),
            _ => None,
        }
    }
}
