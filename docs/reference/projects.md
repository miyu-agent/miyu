# 项目：`project.*`

项目是会话的一个分组（像 ChatGPT、Claude 网页的 Projects）：名称、颜色、新建会话的默认工作目录、人格、预设、附加提示词、留言开关。一个账号自己的一份，存在 `home/<账号>/projects.json`。和代码仓库里的项目配置（`.miyu/config.toml`、回应里的 `untrusted_project`）不是一回事。设计见 `docs/designs/38-项目.md`。英文版：[projects.en.md](projects.en.md)。

| 方法 | 做什么 |
|---|---|
| [`project.list`](#projectlist) | 列出这个账号的项目 |
| [`project.create`](#projectcreate) | 建一个 |
| [`project.update`](#projectupdate) | 改、清掉几格 |
| [`project.delete`](#projectdelete) | 删掉 |

**谁能调**：只给人；扩展调了回 `local_only`。要一直跟着最新的，订阅 `projects` 流（[连接](connection.md)），之后推 `projects.changed`（[推送](pushes.md)）。

## 一项

| 名字 | 说明 |
|---|---|
| `project` | 编号，UUIDv7，核心起 |
| `name` | 名称，1 到 40 个字，首尾不带空白 |
| `description` | 说明，最多 80 个字 |
| `color` | `#rrggbb` |
| `workspace` | 在项目里新建会话的工作目录，人看到的写法 |
| `persona` | 新建会话的人格编号；`null` 是无人格；没有这一格是照配置的默认 |
| `preset` | 新建会话的预设编号；没有是照配置的默认 |
| `prompt` | 附加提示词，最多 8000 个字 |
| `open` | `true`：能和项目外的会话互相留言；`false` 的不写 |
| `created` | 造的时刻 |

没写的格不出现。

## `project.list`

**参数**：无。**回应**：`{"projects": [<一项>…]}`，照造的先后。

## `project.create`

**参数**：上表除 `project`、`created` 以外的格，`name` 必写；写空字符串的格当没写。**回应**：`{"project": <编号>}`。

## `project.update`

**参数**：`project`（必写），要改的格，`unset`：要清掉的格名的数组（`description`、`color`、`workspace`、`persona`、`preset`、`prompt`）。写了值的换成新值，写空字符串的清掉，`unset` 列的回到「没写」。**回应**：`{}`。

## `project.delete`

**参数**：`project`。**回应**：`{}`。

## 出错

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `data.field` 说哪一格：名称空的、太长、首尾带空白；说明、附加提示词太长；颜色不是 `#rrggbb`；工作目录全是空白；`unset` 列了不能清的格、列了又写了值的格 |
| `unknown_project` | 没有这个项目 |
| `unknown_persona`、`persona_invalid`、`unknown_preset`、`preset_invalid` | 写的人格、预设没有、写错了：建、改的时候就查 |
| `internal_error`（`-32603`） | `projects.json` 读不成、写不进 |
