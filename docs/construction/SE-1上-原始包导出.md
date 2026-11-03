# SE-1 上：原始包导出

设计 `58-会话导出和导入.md` 第一节、第六节第 1 条；`07-存储.md` 第三节「导出一个会话」（2026-09-30 项目主人定）。

## 做什么

- `session.export {session, format: "bundle", to: {path}}`：一个会话连着它的全部后代、分叉来源到分叉点为止的那几段，和它们引用到的 blob，打成一个 ZIP 写到人给的路径。只给人的头。
- 存储层 `miyu_store::bundle`：段原文、清单、blob 边拷边核对，先写临时文件再挂上去、不覆盖。

## 我定的（技术题，照推荐）

1. **分叉出来的支线算后代**：索引里支线的 `parent` 是父会话，导父会话时一起带走。导支线时父会话是它的分叉来源，只带到分叉点。
2. **blob 怎么认**：不照每种事件的格一样样列，日志每一行照 JSON 读，写成内容哈希的字都算引用；blob 库里没有的进 `absent`、不算错（`model.called` 的请求哈希这类本来就不是 blob）。新加带 blob 的事件不用改这里。
3. **ZIP 库**：加 `zip` 9.3，只开 deflate，后端用工作区里本来就有的 flate2 + zlib-rs；多进来 `nix`、`typed-path` 两个包（许可证门禁过了）。
4. **不覆盖**：硬链接挂上去（只许新建），系统不支持的退回「没有才改名」。
5. **命令行和 Markdown 挪到 SE-1 中**：命令行要连着帮助页、命令行文案一起做（设计 47），和 Markdown 的几个开关一起定，这一步只交协议。

## 测试

- 存储：`crates/miyu-store/src/bundle/tests.rs`，四条：段原样、半行不要、引用的 blob 进包、不是 blob 的进 `absent`、清单读得回；`upto` 截在那一条、后面的段和那以后引用的 blob 都不要；已经有的不覆盖、不留临时文件；blob 被改过整个失败、不留包。
- 协议：`crates/miyu-endpoint/tests/session_export.rs`，两条：真核心跑两轮、分叉一条支线，导主会话连着支线、段和数据根里的一个字节不差；导支线带来源到分叉点、不带分叉以后的；已经有的、数据根里的、目录不在的、相对路径、`format: markdown`、没有这个会话的都拒。

## 文档

`docs/reference/sessions.md`、`sessions.en.md`、`errors.md`、`errors.en.md`、`methods-v1.txt`；`docs/blueprint/protocol.md`「`session.export`」；`docs/blueprint/store.md`「原始包」；设计 58 第六节照「命令行挪到中」改。
