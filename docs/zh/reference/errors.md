# 出错

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

核心拒绝一个请求时，回应的样子如下：

```json
{"error": {"code": -32010, "data": {"reason": "session_not_found"}, "message": "会话不存在。"}, "id": "c7", "jsonrpc": "2.0"}
```

- `code`：JSON-RPC 自己定义的几种错误使用标准码（下表的前五行）；Miyu 自己的错误一律使用 `-32010`。
- `data.reason`：给程序看的原因码，程序应该根据它来判断。有的错误还另外带几个字段，写在下面的表里。
- `message`：给人看的一句话，使用这个连接的语言（握手以前和握手被拒时，使用英文，或者那一次握手报的语言）。不要根据它来判断。

## 通用

| `reason` | `code` | 什么时候 |
|---|---|---|
| `parse_error` | -32700 | 收到的内容不是 JSON |
| `line_too_long` | -32010 | 一行超过 1 MiB：核心只拒绝这一行，连接仍然保持；`data.limit` 是上限，`id` 是从这一行开头识别出来的那个编号 |
| `invalid_request` | -32600 | 收到的内容是 JSON，但不是请求 |
| `unknown_method` | -32601 | 没有这个方法（可选的软件包没有编译进来时，它的方法也返回这个错误） |
| `bad_params` | -32602 | 参数无法读取、类型不对或者写法不对；各个方法的具体情形写在各自的「出错」里 |
| `internal_error` | -32603 | 核心自己出了问题（磁盘、损坏的文件……），原因记在核心的运行日志里 |
| `hello_first` | -32010 | 在握手以前调用了其他方法 |
| `local_only` | -32010 | 扩展调用了只给人使用的方法；或者登录方式不对 |
| `local_machine_only` | -32010 | 这个方法只能在核心所在的机器上使用：远程连接调用了读取本机文件、提交本机路径这一类方法（见 [连接](connection.md)「本机才能用的」） |
| `not_a_provider`、`not_an_extension` | -32010 | 这个方法只给核心启动的扩展使用，其他连接调用了它 |
| `needs_approval` | -32010 | 扩展需要的能力还没有批准，或者调用的方法所需的能力没有在清单里声明，或者 `provide` 的工具写了 `runs_commands` 而软件包没有获批 `tools.commands`；缺少的能力写在 `data.capabilities` 里 |

## 连接与登录

这一类错误有 `protocol_mismatch`、`bad_token`、`bad_code`、`bad_login`、`bad_password`、`password_not_set`（还没有设置访问密码）、`login_throttled`（这几个错误都在回应以后断开连接）、`setup_first`；`not_permitted`（关在沙盒里的会话通过端口安装、卸载、升级软件包时返回）；`login.redeem` 的票不存在、已经过期或者已经用过时，返回 `bad_ticket`。详细的见 [连接](connection.md)。

## 会话

| `reason` | 什么时候 | `data` |
|---|---|---|
| `session_not_found` | 没有这个会话；已经删除的会话也返回这个错误 | |
| `session_broken` | 会话无法载入：日志损坏，或者无法解析 | |
| `session_read_only` | 会话日志中间有一行损坏：损坏的那一行以前的内容仍然可以用 `view.page` 查看，但是会话无法载入，也不能接着对话（发送消息、订阅都会被拒绝），要继续就新开一个会话。核心不修改、也不修复这份日志 | `segment`（哪一段，只是文件名）、`line`（这一段的第几行） |
| `session_unavailable` | 会话此刻无法读取：读取日志时发生了读写错误（权限不够、打开的文件数用光、磁盘出错），日志本身没有损坏，过一会儿再试即可 | |
| `export_exists` | 导出要写入的文件已经存在，核心不覆盖它 | |
| `export_in_data_root` | 导出不能写进数据目录 | |
| `export_unwritable` | 导出的目录不存在，或者无法写入 | |
| `export_too_big` | 放进回应的导出（`inline`）超过了 512 KiB：需要改成保存到文件 | |
| `bad_bundle` | 导入的内容不是会话包 | |
| `bundle_mismatch` | 导入的会话包已经损坏：哈希或者序号对不上 | |
| `bundle_too_new` | 导入的会话包是由更新的版本打包的 | |
| `session_exists` | 已经有一个编号相同、内容不同的会话 | `session` |
| `archived` | 会话是只读的存档，不能接着对话 | |
| `session_stopped` | 会话已经停止；再发送一次会重新载入会话 | |
| `session_sealed` | 会话已经封存，修改对话的命令不再接受 | |
| `restoring` | 撤回还没有完成时收到了命令 | |
| `venue_session` | 向场所会话发送消息时没有带 `as` | |
| `shell_not_allowed` | `session.send` 的 `shell` 不是人在头（连接核心的界面程序，例如终端界面、网页界面）里输入的（带了 `from`、`as`，由扩展发送，或者是场所会话） | |
| `shell_unavailable` | `session.send` 带了 `shell`，但是没有能运行命令的工具 | |
| `empty_message` | 一句话里一块内容都没有 | |
| `dir_too_wide` | 另外加入的目录太宽 | |
| `not_running` | 打断时没有正在进行的回合 | |
| `nothing_queued`、`already_heard` | 退回排队中的消息（`session.withdraw`）时：没有排队中的消息；要退回的那一条已经被请求带上了 | |
| `turn_running` | 撤回、重做、压缩、清空、删除会话、更换人格或预设时，有回合正在进行 | |
| `id_reused` | 同一个请求编号换了参数再次发送（`session.create` 只比较 `cwd`、`dirs` 以外的参数）；参数相同时返回上一次的回应。核心重启以后不再比较参数 | |
| `unknown_turn` | 要撤回的那一轮不在现在的历史里 | |
| `nothing_to_revert`、`nothing_to_unrevert` | 没有可以撤回的内容；没有可以恢复的撤回 | |
| `not_redoable` | 最后一轮不是由人说话开始的 | |
| `nothing_to_compact`、`nothing_to_clear` | 没有可以压缩的内容；上下文本来就是空的 | |
| `nothing_to_recap`、`recap_failed` | 没有可以回顾的内容；回顾没有生成成功 | |
| `not_asking` | 要回答的那次调用并没有在等待回答 | |
| `no_rule`、`unexpected_reason`、`bad_answer` | 回答确认请求或提问时，写法和当时的情形对不上 | |
| `not_ambient`、`already_answered` | `session.respond` 的 `to` 不对 | `messages` |
| `unknown_call` | `view.detail` 找不到那次调用或者那次压缩 | |
| `unknown_job`、`not_a_command` | 没有这个任务；要读取输出的是子代理 | |
| `goal_exists`、`no_goal`、`goal_paused`、`goal_running`、`goal_unavailable` | 长期目标的命令和目标现在的状态不符；这个会话没有长期目标 | `why` |
| `prefix_changing` | 文件改了、前缀将要改变，开始新一轮之前要先询问人（只对声明了 `caps.prefix_ask` 的头返回） | `files`、`more` |

## 附件与文件

| `reason` | 什么时候 | `data` |
|---|---|---|
| `attachment_unreadable`、`attachment_in_data_root` | `blob.put` 的路径无法读取；路径在数据目录里 | |
| `attachment_too_big` | 附件超过了上限 | |
| `unknown_attachment` | 附件对应的 blob 不存在 | |
| `unknown_blob` | 没有这个 blob，或者这个连接无法读取它 | |
| `unknown_shown` | `session.shown`：没有这一步的展示，或者这个连接无法读取那个会话 | |
| `file_changed`、`file_gone` | `session.shown`：展示的文件的大小或者修改时间变了；文件已经不存在，或者不再是普通文件 | |
| `too_many_uploads`、`upload_unknown` | 同时打开的分块上传太多；上传编号无法识别 | |
| `upload_offset`、`upload_incomplete` | 块的位置不对；还没有收齐全部内容 | `received` |
| `path_unreadable`、`path_forbidden`、`not_a_directory` | 路径不存在或者无法读取；路径在数据目录里；路径是文件而不是目录 | |

## 配置、密钥、模型

| `reason` | 什么时候 | `data` |
|---|---|---|
| `unknown_config_key` | 配置清单里没有这个键 | `problems` |
| `config_invalid` | 值不对，或者这一项不能写在这一层 | `problems` |
| `config_conflict` | `expect` 或者 `version` 对不上 | `current` 或 `version` |
| `config_file_broken` | 配置文件此刻无法读取，所以无法只修改其中几项 | `problems` |
| `no_project_config` | 从这个目录找不到项目配置 | |
| `unknown_file` | `check` 的文件不是 Miyu 读取的那几种 | |
| `unknown_secret` | 没有这个密钥 | |
| `unknown_provider`、`unknown_model` | 不是已经配置好的供应商；无法解析模型 | |
| `provider_exists` | 修改供应商编号（`provider.rename`）时，新编号已经存在 | |
| `preset_exists`、`persona_exists` | `preset.set`、`persona.set` 带 `new: true` 新建时，写的编号已经存在 | |
| `no_model` | 没有可以使用的模型 | `message` |
| `cooling` | 所有候选模型都在冷却 | `message`、`wait_ms` |
| `model_failed` | 请求已经发出，但是出错了 | `class`、`status`、`message` |
| `embedding_off`、`embedding_failed` | 语义模型关闭着；无法计算 | `why` |

## 人格与预设

| `reason` | 什么时候 | `data` |
|---|---|---|
| `unknown_persona`、`unknown_preset` | 没有这个人格或者预设 | |
| `persona_invalid`、`preset_invalid` | 人格或者预设的文件写错了 | `problem`、`message`、`line` |
| `persona_conflict`、`preset_conflict`、`avatar_conflict` | `expect` 对不上（`avatar_conflict` 指的是你的头像） | `current` |
| `preset_fixed` | `preset.set`、`preset.delete` 要修改或删除的预设不能修改（`preset.list`、`preset.get` 里带 `fixed: true` 的） | |
| `nothing_to_delete` | 你那一层本来就没有内容可以删除 | |
| `avatar_not_image`、`avatar_too_big`、`background_not_image`、`background_too_big` | 头像或者背景图不是图片，或者太大 | 量到的和上限 |

## 项目

| `reason` | 什么时候 | `data` |
|---|---|---|
| `unknown_schedule` | `schedule.cancel`、`schedule.pause`、`schedule.resume`、`schedule.update` 时没有这个定时任务 | |
| `too_many_schedules` | `schedule.add` 时定时任务已经有 200 个（人创建的、AI 创建的、软件包创建的合在一起计算，这是熔断性质的安全上限）。需要先删除几个，已有的定时任务仍然照常运行 | |
| `unknown_project` | 没有这个项目：项目的方法，以及 `session.create`、`session.set_meta` 里写的 `project` 都会返回这个错误 | |

## 软件包、扩展、命令

| `reason` | 什么时候 | `data` |
|---|---|---|
| `unknown_package` | 没有这个软件包 | |
| `package_invalid` | 清单写错了 | `problem`、`line` |
| `no_provider`、`package_required` | `bus.call`：总线上没有软件包提供这个名字；`plugs` 的名字没有写明选哪一家 | |
| `package_conflict`、`package_exists` | 软件包带的名字和已有的冲突；和出厂的软件包编号相同 | |
| `package_file_bad`、`wrong_target` | 包文件无法解开；不支持当前系统（清单写的 `targets` 里没有本机这一种，或者 `systems` 里没有本机的系统） | |
| `download_failed`、`source_unreachable` | 从软件源安装时：包文件从哪一处都没有下载成功；一个软件源都没有连上，本机也没有索引 | `why` |
| `depends_missing`、`package_depended` | 依赖没有安装，或者已安装的版本低于下限；其他软件包依赖它 | `missing`、`by` |
| `program_missing` | 找不到软件包的程序 | |
| `not_restartable`、`package_off` | 没有可以重启的内容；软件包关闭着 | |
| `extension_off` | 重启了一个关闭着的扩展 | |
| `no_drivers` | `driver.check` 的软件包没有声明模型驱动 | |
| `driver_busy` | `driver.check` 需要重启扩展，但是此刻有会话正在通过它的驱动调用模型 | `running` |
| `port_taken` | 扩展要登记的端口已经被其他扩展登记了（`port.claim`）；错误里不说明是谁 | |
| `unknown_server` | 系统配置里没有这个 MCP 服务 | |
| `not_a_ui_package`、`prefs_too_big` | `prefs.*`：已安装的软件包不是界面包；合并以后的设置超过 64 KiB，此时什么都没有修改 | `size`、`limit` |
| `oauth_unregistered`、`oauth_failed` | `mcp.login`：没有客户端，授权服务器又不接受动态注册；登录无法继续进行（`mcp.login_finish`：贴回来的地址对不上） | `why` |
| `not_logged_in` | `mcp.logout`：这个服务从来没有登录过 | |
| `not_waiting` | `mcp.login_finish`：这个服务没有正在等待的登录 | |
| `mcp_invalid` | `mcp.set` 提交的服务写错了 | `problem` |
| `server_from_package` | `mcp.set`：系统配置里没有写这个名字，但是有软件包带着同名的服务 | |
| `unknown_skill` | `skill.remove`：没有这个技能 | `name` |
| `skill_from_package` | `skill.remove`：随软件包来的技能不能删除 | `name` |
| `skill_shared` | `skill.remove`：其他目录里的技能（`skills.external.dirs`，`from` 是 `agents`）不通过 Miyu 删除，因为其他客户端也在使用它 | `name` |
| `no_page`、`not_found` | 没有后台页；没有这份文件 | |
| `program_not_running`、`unregistered`、`method_failed`、`method_timeout` | `package.call`：程序没有连接着；方法没有登记；程序返回了错误（程序返回的一行超过 1 MiB 时也算，此时 `code` 是 `line_too_long`）；程序没有回应 | `method`、`message`、`code` |
| `not_exported` | 扩展调用了清单 `[export]` 里没有的方法：用 `bus.call` 时是找到的那一家的方法，用 `package.call` 时是调用自己的方法 | `method` |
| `own_package_only` | 扩展的 `package.call` 写了其他软件包的编号：按软件包编号调用时，扩展只能调用到自己，跨软件包要使用 `bus.call` | `package` |
| `bad_tool` | `provide` 里的一件工具写得不对 | `tool`、`problem` |
| `unknown_widget`、`bad_widget_content` | 界面块不在清单里；内容的写法不对 | `widget`；`path`、`problem` |
| `unknown_command`、`command_not_allowed`、`owner_only` | 没有这个命令；这个人不能使用这个命令；这个命令只给主人本人使用 | `why` |
| `command_unavailable`、`command_failed` | 扩展的命令、MCP 服务的提示词此刻无法使用，或者出了错；哪些情形属于哪一种，见 [`command.run`](packages.md#commandrun) | `why` |

## 记忆与知识库

| `reason` | 什么时候 | `data` |
|---|---|---|
| `memory_not_installed`、`memory_unavailable` | 没有安装记忆；这里没有记忆 | `why` |
| `memory_busy`、`dream_failed` | 记忆正在整理；整理没有成功 | `message` |
| `not_dreaming` | `/dream stop`、`memory.dream_stop`：没有正在运行的整理 | |
| `unknown_memory`、`memory_not_current` | 没有这一条记忆；这一条已经被修改、作废或者清除了 | |
| `memory_too_long` | 记忆超过了 120 个字 | `chars`、`limit` |
| `unknown_kb`、`unknown_document` | 没有这个知识库，或者没有这个文档 | |
| `kb_exists`、`document_exists`、`kb_read_only`、`kb_packaged` | 知识库或者文档已经存在；挂载进来的知识库不能添加、删除文档；软件包带来的知识库不能修改 | |
| `folder_unavailable`、`trash_failed` | 要挂载的目录无法使用；无法移进回收站 | `message` |

## 画图

这一类错误有 `mermaid_too_long`、`mermaid_failed`（带 `detail`）。
