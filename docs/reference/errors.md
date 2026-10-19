# 出错

拒绝的回应：

```json
{"error": {"code": -32010, "data": {"reason": "session_not_found"}, "message": "会话不存在。"}, "id": "c7", "jsonrpc": "2.0"}
```

- `code`：JSON-RPC 自己的几种用标准码（下表前五行）；Miyu 的一律 `-32010`。
- `data.reason`：给程序看的原因码，照它判断。有的另带几格，写在表里。
- `message`：给人看的一句，照这个连接的语言（握手以前和握手被拒时照英文或那一次报的语言）。别拿它判断。

英文版：[errors.en.md](errors.en.md)。

## 通用

| `reason` | `code` | 什么时候 |
|---|---|---|
| `parse_error` | -32700 | 不是 JSON；一行太长（之后断开） |
| `invalid_request` | -32600 | 是 JSON，不是请求 |
| `unknown_method` | -32601 | 没有这个方法（可选的包没编进来的方法也是） |
| `bad_params` | -32602 | 参数读不成、类型不对、写法不对；各方法的具体情形写在各自的「出错」里 |
| `internal_error` | -32603 | 核心自己出了问题（磁盘、坏了的文件……），原因记在核心的运行日志里 |
| `hello_first` | -32010 | 握手以前调了别的方法 |
| `local_only` | -32010 | 只给人的方法，扩展调了；或者登录方式不对 |
| `not_a_provider`、`not_an_extension` | -32010 | 只给核心拉起的扩展的方法，别的连接调了 |
| `needs_approval` | -32010 | 扩展要的能力还没批，`data.capabilities` |
| `no_system_account` | -32010 | 只给系统账号的连接的方法 |

## 连接与登录

`protocol_mismatch`、`bad_token`、`bad_code`、`bad_login`、`bad_password`、`login_throttled`（都回完就断开）、`setup_first`。见 [连接](connection.md)。

## 会话

| `reason` | 什么时候 | `data` |
|---|---|---|
| `session_not_found` | 没有这个会话，删了的也是 | |
| `session_broken` | 会话载入不了：日志坏了、读不了 | |
| `session_stopped` | 会话停了；再发一次会重新载入 | |
| `session_sealed` | 会话封存了，改对话的命令不收 | |
| `restoring` | 撤销还没做完时来的命令 | |
| `venue_session` | 对场所会话说话却没带 `as` | |
| `empty_message` | 一句话一块内容都没有 | |
| `dir_too_wide` | 加进来的目录太宽 | |
| `not_running` | 打断时没有回合在进行 | |
| `turn_running` | 撤销、重做、压缩、清空、删会话、换人格预设时有回合在进行 | |
| `unknown_turn` | 要撤的那一轮不在现在的历史里 | |
| `nothing_to_revert`、`nothing_to_unrevert` | 没有能撤销、能恢复的 | |
| `not_redoable` | 最后一轮不是人说话开的 | |
| `nothing_to_compact`、`nothing_to_clear` | 没有能压缩的；上下文本来就是空的 | |
| `nothing_to_recap`、`recap_failed` | 没什么可回顾的；回顾没写成 | |
| `not_asking` | 回答的调用没在等回答 | |
| `no_rule`、`unexpected_reason`、`bad_answer` | 回答确认、提问的写法和情形对不上 | |
| `not_ambient`、`already_answered` | `session.respond` 的 `to` 不对 | `messages` |
| `unknown_call` | `view.detail` 找不到那次调用、那次压缩 | |
| `unknown_job`、`not_a_command` | 没有这个任务；读输出的是子代理 | |
| `goal_exists`、`no_goal`、`goal_paused`、`goal_running`、`goal_unavailable` | 长期目标的命令和现在的样子不合；这个会话没有长期目标 | `why` |

## 附件与文件

| `reason` | 什么时候 | `data` |
|---|---|---|
| `attachment_unreadable`、`attachment_in_data_root` | `blob.put` 的路径读不了；在数据目录里 | |
| `attachment_too_big` | 超过上限 | |
| `unknown_attachment` | 附件的 blob 不在 | |
| `unknown_blob` | 没有这个 blob，或者这个连接读不到 | |
| `too_many_uploads`、`upload_unknown` | 分块上传开多了；编号不认识 | |
| `upload_offset`、`upload_incomplete` | 块的位置不对；还没收齐 | `received` |
| `path_unreadable`、`path_forbidden`、`not_a_directory` | 路径不在、读不了；在数据目录里；是文件不是目录 | |

## 配置、密钥、模型

| `reason` | 什么时候 | `data` |
|---|---|---|
| `unknown_config_key` | 清单里没有这个键 | `problems` |
| `config_invalid` | 值不对、不能写在这一层 | `problems` |
| `config_conflict` | `expect`、`version` 对不上 | `current` 或 `version` |
| `config_file_broken` | 文件现在读不进来，没法只改几项 | `problems` |
| `no_project_config` | 这个目录找不到项目配置 | |
| `unknown_file` | `check` 的文件不是 Miyu 读的那几种 | |
| `unknown_secret` | 没有这个密钥 | |
| `unknown_provider`、`unknown_model` | 不是配好了的供应商；模型解析不出 | |
| `no_model` | 没有能用的模型 | `message` |
| `cooling` | 候选全在冷却 | `message`、`wait_ms` |
| `model_failed` | 请求发了、出错了 | `class`、`status`、`message` |
| `embedding_off`、`embedding_failed` | 语义模型关着；算不出 | `why` |

## 人格与预设

| `reason` | 什么时候 | `data` |
|---|---|---|
| `unknown_persona`、`unknown_preset` | 没有这个人格、预设 | |
| `persona_invalid`、`preset_invalid` | 文件写错了 | `problem`、`message`、`line` |
| `persona_conflict`、`preset_conflict` | `expect` 对不上 | `current` |
| `nothing_to_delete` | 你那一层本来就没有 | |
| `avatar_not_image`、`avatar_too_big`、`background_not_image`、`background_too_big` | 头像、背景图不是图、太大 | 量到的和上限 |

## 软件包、扩展、命令

| `reason` | 什么时候 | `data` |
|---|---|---|
| `unknown_package` | 没有这个包 | |
| `package_invalid` | 清单写错了 | `problem`、`line` |
| `package_conflict`、`package_exists`、`package_required` | 带的名字撞了；和出厂的同编号；卸必需的 | |
| `depends_missing`、`package_depended` | 依赖没装；别的包依赖它 | `missing`、`by` |
| `program_missing`、`not_switchable` | 程序没找到；没有开关 | |
| `not_restartable`、`package_off` | 没有能重启的；关着 | |
| `extension_off` | 重启一个关着的扩展 | |
| `unknown_server` | 个人设置里没有这个 MCP 服务 | |
| `no_page`、`not_found` | 没有后台页；没有这份文件 | |
| `program_not_running`、`unregistered`、`method_failed`、`method_timeout` | `package.call`：程序没连着；没登记；回了错；没回 | `method`、`message`、`code` |
| `not_exported` | 扩展调了对方没导出的方法 | `method` |
| `bad_tool` | `provide` 的一件工具不对 | `tool`、`problem` |
| `unknown_widget`、`bad_widget_content` | 界面块不在清单里；内容写法不对 | `widget`；`path`、`problem` |
| `unknown_command`、`command_not_allowed`、`owner_only` | 没有这个命令；这个人不能用；只给主人本人 | `why` |
| `command_unavailable`、`command_failed` | 扩展的命令没答、答了错 | `why` |

## 记忆与知识库

| `reason` | 什么时候 | `data` |
|---|---|---|
| `memory_not_installed`、`memory_unavailable` | 没装记忆；这里没有记忆 | `why` |
| `memory_busy`、`dream_failed` | 正在整理；整理没成 | `message` |
| `unknown_memory`、`memory_not_current` | 没有这一条；已经改掉、作废、清掉了 | |
| `memory_too_long` | 超过 120 个字 | `chars`、`limit` |
| `unknown_kb`、`unknown_document` | 没有这个库、这个文档 | |
| `kb_exists`、`document_exists`、`kb_read_only`、`kb_packaged` | 已经有了；挂进来的库不能加删；软件包带来的库不能改 | |
| `folder_unavailable`、`trash_failed` | 要挂的目录用不了；挪不进回收站 | `message` |

## 画图

`mermaid_too_long`、`mermaid_failed`（`detail`）。
