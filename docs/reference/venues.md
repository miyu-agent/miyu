# 场所：通讯平台上的会话

场所是会话发生的地方：本机（`local`），或者通讯平台上的一个私聊、一个群。接入通讯平台的桥（例如接入QQ）是一个扩展，声明了系统账号：群会话归这个系统账号，主人的私聊归主人。这一页列桥要用的方法。英文版：[venues.en.md](venues.en.md)。

| 方法 | 做什么 |
|---|---|
| [`venue.session`](#venuesession) | 找回或者造一个场所的主线会话 |
| [`venue.sessions`](#venuesessions) | 列出这个桥的全部场所会话 |
| [`venue.binding`](#venuebinding) | 问一个平台身份对着哪个本机账号 |
| [`venue.plugins`](#venueplugins) | 说一个场所开着哪些场所插件 |
| [`session.send` 的 `as`、`venue`](#sessionsend-的-asvenue) | 代表平台上的人说话 |
| [`session.respond`](#sessionrespond) | 照旁听记下的几条开一轮 |
| [`session.note`](#sessionnote) | 只记几块事实，不开回合 |
| [`venue.records`](#venuerecords) | 判官看的群聊记录 |
| [`events.append` 的 `venue.*`](#eventsappend-的-venue) | 记撤回、记她发出去的话 |

**谁能调**：`venue.session`、`venue.sessions`、`venue.binding`、`venue.plugins` 只给系统账号的连接（声明了系统账号的包的扩展）和每人一份的包（`[process] scope = "account"`）的那一份，别的回 `no_system_account`。每人一份的那一份：场所会话一律归它替的那个人（私聊的对方在主人对应表里也一样）；主人对应表只在主人自己那一份里认，成员那一份问 `venue.binding` 是 `null`、他名下的会话里 `as` 记成外部身份不带账号；`venue.sessions` 只列他名下这个包的平台的。代表平台上的人说话（带 `as` 的 `session.send`、`command.run`）、`session.note`、`venue.*`、记 `venue.*` 事件都要能力 `act_for_external`。

**场所编号**：桥自己定的短名字（1 到 128 字节，没有控制字符），例如 `qq:private:10001`、`qq:group:123456`。核心不解读，只认第一个 `:` 前面的平台前缀。

**自己的号**（施工 BD-1）：每个人在个人设置里写 `external.me = ["qq:12345"]`，说这几个号是他自己。只在他名下的场所会话里认：他那一份每人一份的包问 `venue.binding` 回他本人，他群里这个号说的话记成他本人。平台桥名下的会话不认。

**主人对应表**：系统配置 `[external.bindings]`，写了的平台身份就是主人（只有一个主人，值写什么都算，空的不算）：

```toml
[external.bindings]
"qq:10001" = "admin"
```

私聊里这个号就是主人本人；群里这个号是主人。

## `venue.session`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `venue` | 字符串，必写 | 场所编号 |
| `kind` | `private` 或 `group`，必写 | 私聊还是群 |
| `peer` | 字符串 | 私聊的对方，`private` 必写，`group` 不能写 |
| `persona` | 字符串或 `null` | 造的时候用哪个人格（桥照自己的规则算好）；`null` 是不用人格 |
| `preset` | 字符串 | 造的时候用哪个预设 |
| `profile` | 布尔 | 造的时候带不带主人写的「关于你」：主人自己的私聊才写 `true` |
| `images` | 布尔，默认 `true` | 这个场所显示得了图片吗 |
| `titles` | 布尔，默认 `true` | 这个会话自动起标题吗：`false` 的从此不自动起，人手改照旧改得了。找回已有的会话时不看 |
| `cwd` | 字符串 | 造的时候的工作目录，不写是属主的默认工作区 |
| `recent` | 整数，1 到 1000 | 只给群：开一轮时群聊近况最多带几条（从最新往前），字节预算照旧做上限、两个取少的；不写的只照字节预算。造的时候钉死 |

造的时候用的几格，找回的不看。

**回应**：`{"session", "created", "account"}`：会话编号、是不是新造的、属主。私聊的对方在对应表里的，属主是那个本机账号；别的归桥的系统账号（`account` 是桥自己的账号，就是陌生人的私聊）。

场所会话没有人能确认：要确认的一律拒绝。同一个 `id` 再发交回同一个。

**出错**：`no_system_account`；`unknown_persona`、`unknown_preset`、`preset_invalid`；`bad_params`。

## `venue.sessions`

**参数**：没有。**回应**：`{"sessions": [{"session", "venue"}…]}`，从新到旧。列的是：归这个系统账号的场所会话，和对应表里这个平台的身份对着的主人的私聊（照清单 `[connection] platform` 认平台，能写几个）。一个场所只列最新的那一个。桥起来时照它订阅。

## `venue.binding`

**参数**：`id`：平台身份。**回应**：`{"account": <本机账号>}`，不在对应表里的是 `null`。照这一刻的配置答。桥挡「只给主人」的动作时用：动谁之前先认目标。

## `venue.plugins`

**参数**：`venue`：场所；`on`：开着的场所插件的名字（同 `provide` 的 `plugin`），不写或空的是全不给。**回应**：`{}`。

整份替换。写了 `plugin` 的工具在这个场所的会话里只有开着才给，下一轮生效；这一轮里刚关掉的再调回「暂时不可用」。核心只在内存里记，重启以后哪个场所都不给：桥连上以后对每个场所发一次。只管自己平台的场所（照清单 `[connection] platform`，能写几个），别的平台的 `bad_params`（`data.field` 是 `venue`），名字写法不对的 `bad_params`（`data.field` 是 `on`）。

## `session.send` 的 `as`、`venue`

场所会话只收带 `as` 的话（不带的回 `venue_session`），本机的会话写了 `as` 是 `bad_params`。

**`as`**：`{"external": <平台身份>, "role": "manager" | "member"}`。`role` 默认 `member`，斜杠命令照它判谁能用；主人不靠 `role` 认，照对应表认。

**`venue`**：一条平台消息的那几格，原样记下：

| 名字 | 说明 |
|---|---|
| `msg` | 平台的消息编号，必写 |
| `reply_to` | 引用的那一条的编号 |
| `name` | 发的人此刻在这里叫什么（群名片，没有的用昵称），最多 64 个字 |
| `my_name` | 她此刻在这里叫什么（群名片，空的用昵称；私聊也是昵称），最多 64 个字。记忆照它和人格的名字认她：说的是这个名字的事记成她自己的，不当成认识的人 |
| `place` | 这个群此刻叫什么（群名），最多 64 个字；私聊不写。只给记忆：群里记下的记忆带上它，她看到的话里没有它 |
| `mentions` | @ 了谁（平台身份的列表） |
| `mentions_me`、`mentions_all` | @ 了她；@ 了全体成员 |
| `media` | 带的东西 `[{"kind", "id", "name"?, "size"?, "shown"?}]`：`kind` 是 `image`、`file`、`voice`、`video`、`sticker`；`id` 是平台的编号（不下载）；`shown` 表示这一样的原图已经当附件交进来了 |
| `forwards` | 合并转发：`[{"nodes": [{"name", "text", "media"?, "forwards"?}], "more"?}]`，套最多 3 层；整条最多 100 条、名字和正文合起来 16 KiB（超了 `bad_params`，`data.limit` 是 `forward_nodes` 或 `forward_bytes`）。正文里要有占位（例如 `[forward]`） |
| `ambient` | 旁听：只记下，不开回合 |
| `asleep` | 睡着时收到的 |
| `backfill` | 补拉回来的（桥断线时漏掉、从平台补回来的）：只进聊天记录，不进群聊近况、判官看的记录；要带着 `ambient` |
| `sent` | 平台给的时刻，写法同事件的 `at`（`2026-10-20T04:30:00.000Z`）；`history` 照它显示。日志照记下的先后，序号不往前挪 |
| `show_ids` | 渲染这一条时写不写发的人的平台身份 |

**记成谁**：私聊里对应表认出的本人是 `{"kind": "person", "account", "via"}`；别的是 `{"kind": "external", "venue", "id", "role"}`，群里的主人另带 `account`。

**命令编号**：桥照「平台、登录的账号、消息编号、平台给的时刻」拼 `id`，断线重发、平台重发都只生效一次。

## `session.respond`

照旁听记下的几条开一轮（群里判了要回的时候）。

**参数**：`session`；`to`：那几条 `message.user` 的序号，1 到 64 条；`facts`：可不写，几块事实 `[{"kind", "text"}]`（`text` 最多 4 KiB），排在触发前面。

**回应**：`{"events": [...]}`。正在跑一轮的并进这一轮，下一步就听到。

**出错**：`not_ambient`（`to` 里有不是旁听记下的，`data.messages`）；`already_answered`（已经当过触发的，`data.messages`）；`bad_params`。

## `session.note`

只记几块事实，不开回合、不打断、不叫醒（例如她的话发不出去，下一步要知道）。

**参数**：`session`；`facts`：1 到 16 块 `{"kind", "text"}`。**回应**：`{"events": [...]}`。正在跑一轮的跟着这一轮，下一次请求就看到；空闲的下一轮开头看到。

## `venue.records`

判官（判群里这一条要不要回的那个小请求）看的群聊记录，和她看到的一行同一个写法。

**参数**：`session`；`msg`：要判的那一条的序号（带 `venue` 的 `message.user`）；`count`：1 到 100；`grow`：可选，起点对齐的步长，至少 1，`count + grow` 不过 100。写了 `grow` 的，起点停在 `grow` 的倍数上（`(之前的条数 − count) / grow × grow`，不到 0 的从头），条数在 `count` 到 `count + grow − 1` 之间：起点一段时间不动，判官的请求前缀稳，缓存命中得上。

**回应**：`{"records": "<之前的几条，一行一条>", "current": "<那一条>", "count": <之前的记录有几条>}`。她回过的话写成 `[you]` 行；撤回的照样标。`current` 引用了别的，编号后面写引用的那条的发的人和内容（`reply-to: msg=8805 小林: @Miyu 在吗`）；日志里没有那一条的只写编号。

**出错**：`bad_params`；`session_not_found`。

## `events.append` 的 `venue.*`

除了自己的 `ext.<包>.*`（见 [扩展](extensions.md#eventsappend)），桥还能往场所会话里记两种：

| `kind` | `body` | 是什么 |
|---|---|---|
| `venue.recalled` | `{"msg", "by"}` | 有人撤回了一条：平台编号、谁撤的 |
| `venue.delivered` | `{"line", "turn", "to", "msg", "text", "images"?}` | 她的话发出去了：哪个会话、哪一轮、回给谁（平台身份的列表）、平台编号、正文、图的哈希 |

这两种只收场所会话的，写法不对的 `bad_params`。她之后看到的群聊里，撤回的标出来，`[you]` 行是她说过的话。
