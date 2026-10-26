# Miyu 核心协议参考

写给接入核心的程序：终端、网页这类「头」，和核心拉起的扩展。只讲现在能用的方法怎么调、回什么、会怎么出错，不讲为什么这样定、哪一步做的（那些在 `docs/blueprint/`）。英文版：[README.en.md](README.en.md)。这份参考用 Apache-2.0（仓库根目录 `LICENSES/Apache-2.0.txt`）：照着它另写客户端、扩展，用什么许可证都行。

## 约定

- **一行一条**：核心和对面在一条连接上互发 JSON-RPC 2.0，一条消息一行（UTF-8，`\n` 结尾），一行最长 1 MiB。
- **先握手**：连上以后第一条必须是 `hello`，握手以前别的方法都回 `hello_first`。
- **字段**：参数是对象。写 `null` 等于没写。不认识的字段不理。
- **出错**：JSON-RPC 自己的几种用它的标准码；Miyu 的一律 `-32010`，原因在 `error.data.reason`，有的另带几格（写在各方法的「出错」里）。`error.message` 是给人看的一句，照握手时报的语言写，别拿它判断。
- **谁能调**：每个方法写明「人」（头的连接）和「扩展」（核心拉起的包的连接）能不能调、有什么不同。
- **协议版本 1 是冻结的**（2026-10-24）：方法列在 [methods-v1.txt](methods-v1.txt)，加上握手的 `hello` 和 `subscribe`、`unsubscribe`、`provide`。以后只加不改不删：已有的方法不删、不改名、不改意思，回应只加字段；新加的方法、字段会写明从哪一版核心起有，老核心对新方法回 `unknown_method`、对不认识的字段不理，所以用之前看一眼握手回应里核心的版本，或者照 `unknown_method` 退回去。删、改名、改意思的只能升到协议 2：握手照你给的 `[最低, 最高]` 谈，谈不拢回 `protocol_mismatch`。

## 目录

| 页 | 方法 |
|---|---|
| [连接](connection.md) | 怎么连、一行一条、`hello`、`subscribe`、`unsubscribe`、`account.*`、`login.*` |
| [会话](sessions.md) | `session.*`、`view.page`、`view.detail` |
| [视图流](view.md) | 给界面直接画的条目、会话状态 |
| [事件](events.md) | 会话日志里的每一种事件 |
| [推送](pushes.md) | 各条流推什么 |
| [附件与文件内容](blob.md) | `blob.*` |
| [文件](fs.md) | `fs.*` |
| [配置、密钥、模型](config.md) | `config.*`、`secret.*`、`provider.*`、`model.*`、`usage.query` |
| [人格与预设](personas.md) | `persona.*`、`preset.*` |
| [项目](projects.md) | `project.*` |
| [软件包与命令](packages.md) | `package.*`、`extension.*`、`mcp.*`、`command.*`、`check` |
| [扩展](extensions.md) | 拉起、能力、`provide`、`tool.call`、`user.append`、`system.append`、界面块、`content.*`、`events.append` |
| [场所](venues.md) | `venue.*`、`session.respond`、`session.note`、代表平台上的人说话 |
| [记忆与知识库](memory.md) | `memory.*`、`kb.*` |
| [其他](misc.md) | `job.*`、`human.get`、`link.preview`、`mermaid.render` |
| [出错](errors.md) | 全部原因码 |

写软件包、扩展、脚本、技能的教程在 [`docs/authors/`](../authors/README.md)。
