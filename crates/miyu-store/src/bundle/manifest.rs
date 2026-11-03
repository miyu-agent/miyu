//! 原始包的清单 `manifest.json`（施工 SE-1 上）：格式版本、谁导的、主会话、每个会话的各段和它们的哈希、包里的 blob、引用到
//! 却不在 blob 库里的哈希。导入（SE-1 下）先照它核对包里的每一项，对不上整个拒。

use serde::{Deserialize, Serialize};

use miyu_kernel::id::{ContentHash, SessionId};

/// 清单的格式版本。改了写法加一；导入认不得比自己新的（`bundle_too_new`）。
pub const FORMAT: u32 = 1;

/// 整份清单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// 格式版本，[`FORMAT`]。
    pub format: u32,
    /// 导它的核心的版本。
    pub miyu: String,
    /// 导出的时刻，RFC 3339。
    pub exported: String,
    /// 主会话：导的是哪一个。
    pub main: SessionId,
    /// 包里的会话：主会话、它的全部后代、分叉来源（到分叉点为止），照导的先后。
    pub sessions: Vec<ManifestSession>,
    /// 包里的 blob，`blobs/<64 位>`。
    pub blobs: Vec<ManifestBlob>,
    /// 日志里写成内容哈希、blob 库里却没有的：多半本来就不是 blob（例如请求字节的哈希）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub absent: Vec<ContentHash>,
}

/// 包里的一个会话。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestSession {
    /// 会话编号，包里是 `sessions/<编号>/`。
    pub id: SessionId,
    /// 取到的最后一条的序号；一条都没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upto: Option<u64>,
    /// 各段，照名字排。
    pub segments: Vec<ManifestSegment>,
}

/// 一段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestSegment {
    /// 段名：第一条的序号补零到 12 位加 `.jsonl`。
    pub name: String,
    /// 包里这一段有多少字节。
    pub bytes: u64,
    /// 包里这一段的 SHA-256。
    pub sha256: ContentHash,
}

/// 包里的一个 blob。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestBlob {
    /// 内容哈希，也是它的名字。
    pub hash: ContentHash,
    /// 多少字节。
    pub bytes: u64,
}
