# Miyu 核心协议参考

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这份参考写给接入核心的程序：「头」（连接核心的界面程序，例如终端界面、网页界面），以及核心拉起的扩展。它只讲现在能用的方法怎样调用、返回什么、会怎样出错，不讲为什么这样设计、在哪一步实现（这些写在 `internal/blueprint/` 里）。这份参考使用 Apache-2.0 许可证（见仓库根目录的 `LICENSES/Apache-2.0.txt`），所以你照着它另写客户端、扩展时，可以使用任何许可证。

## 约定

- **一行一条**：核心和对方在一条连接上互相发送 JSON-RPC 2.0 消息，每条消息占一行（UTF-8 编码，以 `\n` 结尾），一行最长 1 MiB。
- **先握手**：连接以后发送的第一条消息必须是 `hello`；握手以前调用其他方法都会返回 `hello_first`。
- **字段**：参数是一个对象。写 `null` 和不写这个字段的效果相同。核心不识别的字段会被忽略。
- **出错**：JSON-RPC 自己定义的几种错误使用它的标准错误码；Miyu 自己的错误一律使用 `-32010`，原因写在 `error.data.reason` 里，有的错误还另外带几个字段（写在各个方法的「出错」里）。`error.message` 是给人看的一句话，使用握手时报告的语言书写，不要拿它来判断是什么错误。
- **谁能调用**：每个方法都写明了「人」（头的连接）和「扩展」（核心拉起的软件包的连接）能不能调用它，以及两者调用时有什么不同。
- **协议版本 1 是冻结的**：它的方法列在 [methods-v1.txt](../../../internal/blueprint/methods-v1.txt) 里，另外还有握手用的 `hello`，以及 `subscribe`、`unsubscribe`、`provide`。协议 1 以后只增加、不修改、不删除：已有的方法不删除、不改名、不改变意思，回应只增加字段；新加的方法和字段会写明从哪一版核心开始提供。老版本的核心对新方法返回 `unknown_method`，对不识别的字段直接忽略，所以使用新方法和新字段之前，先看一眼握手回应里核心的版本，或者在收到 `unknown_method` 时退回原来的做法。删除、改名、改变意思的改动只能放到协议 2 里：握手时核心按你给出的 `[最低, 最高]` 版本范围协商，协商不成就返回 `protocol_mismatch`。

## 目录

| 页 | 方法 |
|---|---|
| [连接](connection.md) | 怎样连接、一行一条消息、`hello`、`subscribe`、`unsubscribe`、`account.*`、`login.*` |
| [会话](sessions.md) | `session.*`、`view.page`、`view.detail` |
| [视图流](view.md) | 可以直接画到界面上的条目、会话的状态 |
| [事件](events.md) | 会话日志里的每一种事件 |
| [推送](pushes.md) | 每一条流推送什么 |
| [附件与文件内容](blob.md) | `blob.*` |
| [文件](fs.md) | `fs.*` |
| [配置、密钥、模型](config.md) | `config.*`、`secret.*`、`provider.*`、`model.*`、`usage.*` |
| [人格与预设](personas.md) | `persona.*`、`preset.*` |
| [项目](projects.md) | `project.*` |
| [头自己的设置](prefs.md) | `prefs.*` |
| [定时任务](schedules.md) | `schedule.*` |
| [软件包与命令](packages.md) | `package.*`、`extension.*`、`mcp.*`、`command.*`、`check` |
| [总线](bus.md) | `bus.*`、公认的名字 |
| [扩展](extensions.md) | 拉起扩展、能力、`provide`、`tool.call`、`user.append`、`tool.guard`、`tool.after`、`system.append`、界面块、`content.*`、`events.append` |
| [场所](venues.md) | `venue.*`、`session.respond`、`session.note`、代表平台上的人发言 |
| [记忆与知识库](memory.md) | `memory.*`、`kb.*` |
| [其他](misc.md) | `job.*`、`human.get`、`link.preview`、`mermaid.render` |
| [出错](errors.md) | 全部的原因码 |

编写软件包、扩展、脚本、技能的教程，可以看 [开发者文档](../dev/README.md)。
