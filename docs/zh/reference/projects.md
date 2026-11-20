# 项目：`project.*`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

项目是会话的一个分组（类似 ChatGPT、Claude 网页版里的 Projects），包含名称、颜色、新建会话时默认的工作目录、人格、预设，以及附加提示词和留言开关。项目保存在 `projects.json` 里。它和代码仓库里的项目配置（`.miyu/config.toml`、回应里的 `untrusted_project`）不是一回事。

| 方法 | 做什么 |
|---|---|
| [`project.list`](#projectlist) | 列出全部项目 |
| [`project.create`](#projectcreate) | 创建一个项目 |
| [`project.update`](#projectupdate) | 修改几个字段，或者清掉几个字段 |
| [`project.delete`](#projectdelete) | 删除一个项目 |

**谁能调用**：只有人可以调用；扩展调用时返回 `local_only`。头（连接核心的界面程序，例如终端界面、网页界面）如果要一直显示最新的项目，可以订阅 `projects` 流（见 [连接](connection.md)），之后核心会推送 `projects.changed`（见 [推送](pushes.md)）。

## 一项

| 名字 | 说明 |
|---|---|
| `project` | 项目编号，是一个 UUIDv7，由核心生成。 |
| `name` | 项目名称，长度是 1 到 40 个字，开头和结尾不带空白。 |
| `description` | 项目说明，最多 80 个字。 |
| `color` | 项目的颜色，写成 `#rrggbb`。 |
| `workspace` | 在项目里新建会话时使用的工作目录，使用人看到的写法。 |
| `persona` | 新建会话时使用的人格编号；`null` 表示不用人格；没有这一项时使用配置里的默认人格。 |
| `preset` | 新建会话时使用的预设编号；没有这一项时使用配置里的默认预设。 |
| `prompt` | 附加提示词，最多 8000 个字。 |
| `open` | 为 `true` 时，项目里的会话可以和项目外的会话互相留言；为 `false` 时不写这一项。 |
| `created` | 项目创建的时刻。 |

没有填写的字段不会出现。

## `project.list`

**参数**：无。**回应**：`{"projects": [<一项>…]}`，按创建的先后排列。

## `project.create`

**参数**：上表中除了 `project`、`created` 以外的字段，其中 `name` 必写；写成空字符串的字段当作没有填写。**回应**：`{"project": <编号>}`。

## `project.update`

**参数**：`project`（必写）、要修改的字段，以及 `unset`：要清掉的字段名组成的数组（可以是 `description`、`color`、`workspace`、`persona`、`preset`、`prompt`）。写了值的字段换成新值，写成空字符串的字段被清掉，`unset` 里列出的字段回到「没写」的状态。**回应**：`{}`。

## `project.delete`

**参数**：`project`。**回应**：`{"moved": <挪出来的会话个数>}`。核心先给项目里的每个会话写一条 `session.meta_changed {"project": ""}`，把它们逐个挪出项目，会话本身不会被删除。

## 出错

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `data.field` 指出是哪个字段出错：名称是空的、太长，或者开头结尾带空白；说明或者附加提示词太长；颜色不是 `#rrggbb`；工作目录全是空白；`unset` 列了不能清掉的字段，或者列了同时又写了值的字段。 |
| `unknown_project` | 没有这个项目。 |
| `unknown_persona`、`persona_invalid`、`unknown_preset`、`preset_invalid` | 写的人格或者预设不存在，或者写错了：创建和修改项目时就会检查。 |
| `internal_error`（`-32603`） | `projects.json` 无法读取，或者无法写入。 |
