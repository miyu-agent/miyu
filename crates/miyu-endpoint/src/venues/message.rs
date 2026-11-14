//! `session.send` 的 `venue`（施工 O-13 上，`docs/blueprint/chat.md` 第七条第 2 条）：通讯平台上的一条消息的那几格，查过写法
//! 换成内核的 [`VenueMessage`]，原样记进 `message.user`。写错的整条 `bad_params`。合并转发的内容（施工 O-34）查写法和上限：
//! 单层的照查，整条消息合起来的另有上限（`docs/blueprint/venues.md`「场所的格」）。引用的那一条的内容 `quoted`（施工 QT-1）
//! 和转发里的一条同一个形状、同一套查法，套在里面的转发从第一层数，条数、字节数算进同一个上限。

use serde::Deserialize;

use miyu_kernel::event::{Forward, ForwardNode, Media, MediaKind, VenueMessage};
use miyu_kernel::id::ExternalId;
use miyu_kernel::time::Timestamp;

use crate::refusal::Refusal;

/// 平台给的编号最长几个字符。
const ID: usize = 128;
/// 发的人的名字最长几个字符。
const NAME: usize = 64;
/// 文件名、表情的字最长几个字符。
const MEDIA_NAME: usize = 200;
/// 合并转发最多套几层（施工 O-34）：`venue.forwards` 是第一层。
const FORWARD_DEPTH: usize = 3;
/// 一个合并转发最多几条（施工 O-34）。
const FORWARD_NODES: usize = 100;
/// 合并转发里一条的正文最多几个字节（施工 O-34）。
const FORWARD_TEXT: usize = 4096;
/// 一条场所消息里所有转发（连套在里面的）合起来最多几条（施工 O-34，核心审形状时定的第 1 条）。
const ALL_NODES: usize = 100;
/// 一条场所消息里所有转发的名字和正文合起来最多几个字节（同上）。
const ALL_BYTES: usize = 16 * 1024;

/// `venue` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VenueMessageParams {
    msg: String,
    #[serde(default)]
    reply_to: Option<String>,
    #[serde(default)]
    name: Option<String>,
    /// 她此刻在这个场所里叫什么（施工 E-1 修三）：同 `name` 的规矩。
    #[serde(default)]
    my_name: Option<String>,
    /// 这个场所此刻叫什么（施工 E-4 上）：群名，同 `name` 的规矩。
    #[serde(default)]
    place: Option<String>,
    #[serde(default)]
    mentions: Vec<String>,
    #[serde(default)]
    mentions_me: bool,
    #[serde(default)]
    mentions_all: bool,
    #[serde(default)]
    media: Vec<MediaParams>,
    /// 合并转发（施工 O-34）：写了就不能是空的列表，`Option` 才分得出没写和写了空的。
    #[serde(default)]
    forwards: Option<Vec<ForwardParams>>,
    /// 引用的那一条的内容（施工 QT-1）：同转发里的一条，要带着 `reply_to`。
    #[serde(default)]
    quoted: Option<NodeParams>,
    #[serde(default)]
    ambient: bool,
    #[serde(default)]
    asleep: bool,
    /// 补拉回来的（施工 O-40）：要带着 `ambient`。
    #[serde(default)]
    backfill: bool,
    /// 平台给的时刻（施工 O-40），写法同事件的 `at`。
    #[serde(default)]
    sent: Option<String>,
    #[serde(default)]
    show_ids: bool,
}

/// 一样带的东西。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaParams {
    kind: MediaKindParam,
    id: String,
    #[serde(default)]
    name: Option<String>,
    /// 多少字节（施工 O-33）：非负整数，不是的整条 `bad_params`。
    #[serde(default)]
    size: Option<u64>,
    /// 原图已经当图片块交进来了（施工 O-35）：只在 `venue.media` 里有意义，转发里的写了整条 `bad_params`。
    #[serde(default)]
    shown: bool,
    /// 在转发里的编号（施工 O-61）：正整数，只在转发里有意义，`venue.media` 里的写了整条 `bad_params`。
    #[serde(default)]
    index: Option<std::num::NonZeroU32>,
}

/// 一个合并转发（施工 O-34）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ForwardParams {
    nodes: Vec<NodeParams>,
    /// 没放进来的条数：写了就要大于 0。
    #[serde(default)]
    more: Option<u64>,
}

/// 合并转发里的一条（施工 O-34）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeParams {
    name: String,
    text: String,
    #[serde(default)]
    media: Vec<MediaParams>,
    #[serde(default)]
    forwards: Option<Vec<ForwardParams>>,
}

/// 带的东西是哪一种：只认这五种。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MediaKindParam {
    Image,
    File,
    Voice,
    Video,
    Sticker,
}

impl VenueMessageParams {
    /// 查过写法，换成内核的那一份。
    pub(crate) fn checked(self) -> Result<VenueMessage, Refusal> {
        // 编号只在转发里有（施工 O-61）：转发外面的照 `fetch_media` 自己数。
        if self.media.iter().any(|item| item.index.is_some()) {
            return Err(Refusal::BAD_PARAMS);
        }
        let media = self
            .media
            .into_iter()
            .map(MediaParams::checked)
            .collect::<Result<Vec<_>, Refusal>>()?;
        // 引用的内容挂在引用的那一行下面：没写引用的是写错了（施工 QT-1）。
        if self.quoted.is_some() && self.reply_to.is_none() {
            return Err(Refusal::BAD_PARAMS);
        }
        let mut tally = Tally::default();
        let forwards = match self.forwards {
            Some(forwards) => forwarded(forwards, 1, &mut tally)?,
            None => Vec::new(),
        };
        let quoted = self
            .quoted
            .map(|quoted| node(quoted, 1, &mut tally).map(Box::new))
            .transpose()?;
        tally.within()?;
        Ok(VenueMessage {
            msg: platform_id(self.msg)?,
            reply_to: self.reply_to.map(platform_id).transpose()?,
            name: self.name.map(|name| words(name, NAME)).transpose()?,
            my_name: self.my_name.map(|name| words(name, NAME)).transpose()?,
            place: self.place.map(|name| words(name, NAME)).transpose()?,
            mentions: self
                .mentions
                .iter()
                .map(|who| ExternalId::parse(who).map_err(|_| Refusal::BAD_PARAMS))
                .collect::<Result<_, _>>()?,
            mentions_me: self.mentions_me,
            mentions_all: self.mentions_all,
            media,
            forwards,
            quoted,
            ambient: self.ambient,
            asleep: self.asleep,
            // 补拉回来的只记下：不带 `ambient` 的会开回合，写错了（施工 O-40）。
            backfill: match (self.backfill, self.ambient) {
                (true, false) => return Err(Refusal::BAD_PARAMS),
                (backfill, _) => backfill,
            },
            sent: self
                .sent
                .map(|sent| Timestamp::parse(&sent).map_err(|_| Refusal::BAD_PARAMS))
                .transpose()?,
            show_ids: self.show_ids,
        })
    }
}

impl MediaParams {
    /// 查过写法，换成内核的那一份。
    fn checked(self) -> Result<Media, Refusal> {
        Ok(Media {
            kind: match self.kind {
                MediaKindParam::Image => MediaKind::Image,
                MediaKindParam::File => MediaKind::File,
                MediaKindParam::Voice => MediaKind::Voice,
                MediaKindParam::Video => MediaKind::Video,
                MediaKindParam::Sticker => MediaKind::Sticker,
            },
            id: platform_id(self.id)?,
            name: self.name.map(|name| words(name, MEDIA_NAME)).transpose()?,
            size: self.size,
            shown: self.shown,
            index: self.index,
        })
    }
}

/// 一条消息里的转发合起来有多少（施工 O-34）：条数，名字和正文的字节数。
#[derive(Debug, Default)]
struct Tally {
    nodes: usize,
    bytes: usize,
}

impl Tally {
    /// 合起来超没超整条消息的上限：超了的 `bad_params`，`data.limit` 说是哪一样（`forward_nodes`、`forward_bytes`）。
    fn within(&self) -> Result<(), Refusal> {
        if self.nodes > ALL_NODES {
            return Err(Refusal::bad_params_about("forward_nodes"));
        }
        if self.bytes > ALL_BYTES {
            return Err(Refusal::bad_params_about("forward_bytes"));
        }
        Ok(())
    }
}

/// 第 `depth` 层的几个转发（施工 O-34）：不是空的列表，不超过 [`FORWARD_DEPTH`] 层；每个 1 到 [`FORWARD_NODES`] 条、`more`
/// 写了就大于 0；每条照 [`node`] 查。条数、字节数记进 `tally`。
fn forwarded(
    forwards: Vec<ForwardParams>,
    depth: usize,
    tally: &mut Tally,
) -> Result<Vec<Forward>, Refusal> {
    if forwards.is_empty() || depth > FORWARD_DEPTH {
        return Err(Refusal::BAD_PARAMS);
    }
    forwards
        .into_iter()
        .map(|forward| {
            if forward.nodes.is_empty()
                || forward.nodes.len() > FORWARD_NODES
                || forward.more == Some(0)
            {
                return Err(Refusal::BAD_PARAMS);
            }
            let nodes = forward
                .nodes
                .into_iter()
                .map(|one| node(one, depth + 1, tally))
                .collect::<Result<_, _>>()?;
            Ok(Forward {
                nodes,
                more: forward.more,
            })
        })
        .collect()
}

/// 一条（转发里的、引用的那一条，施工 O-34、QT-1）：名字同发的人的名字，正文最多 [`FORWARD_TEXT`] 字节（换行这些照收，渲染时
/// 转义），带的东西同 `media`、不能写 `shown`（施工 O-35）。套在里面的转发是第 `depth` 层。条数、字节数记进 `tally`。
fn node(node: NodeParams, depth: usize, tally: &mut Tally) -> Result<ForwardNode, Refusal> {
    // 转发里的东西没有当原图交进来的（施工 O-35）：写了 `shown` 的是写错了。
    if node.text.len() > FORWARD_TEXT || node.media.iter().any(|item| item.shown) {
        return Err(Refusal::BAD_PARAMS);
    }
    let name = words(node.name, NAME)?;
    tally.nodes += 1;
    tally.bytes += name.len() + node.text.len();
    Ok(ForwardNode {
        name,
        text: node.text,
        media: node
            .media
            .into_iter()
            .map(MediaParams::checked)
            .collect::<Result<_, _>>()?,
        forwards: match node.forwards {
            Some(inner) => forwarded(inner, depth, tally)?,
            None => Vec::new(),
        },
    })
}

/// 平台给的编号：不是空的，最多 [`ID`] 个字符，没有控制字符（`events.append` 的场所事件也照它查）。
pub(crate) fn platform_id(text: String) -> Result<String, Refusal> {
    let fits =
        !text.is_empty() && text.chars().count() <= ID && !text.chars().any(char::is_control);
    fits.then_some(text).ok_or(Refusal::BAD_PARAMS)
}

/// 给人看的一句：去掉前后空白不是空的，最多 `most` 个字符，没有控制字符。原样记。
fn words(text: String, most: usize) -> Result<String, Refusal> {
    let fits = !text.trim().is_empty()
        && text.chars().count() <= most
        && !text.chars().any(char::is_control);
    fits.then_some(text).ok_or(Refusal::BAD_PARAMS)
}
