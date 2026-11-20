# 场所：通讯平台上的会话

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

场所是会话发生的地方：可以是本机（`local`），也可以是通讯平台上的一个私聊或者一个群。接入通讯平台的桥（例如接入 QQ 的桥）是一个扩展，它的清单里 `[connection] platform` 写着它接入的平台。场所会话都归主人所有：如果私聊的对方在主人对应表里（也就是主人本人），这个私聊按照本机会话对待；群和陌生人的私聊关在沙盒里。这一页列出桥要使用的方法。

| 方法 | 做什么 |
|---|---|
| [`venue.session`](#venuesession) | 找回或者创建一个场所的主线会话 |
| [`venue.sessions`](#venuesessions) | 列出这个桥的全部场所会话 |
| [`venue.binding`](#venuebinding) | 查询一个平台身份是不是主人 |
| [`venue.plugins`](#venueplugins) | 告诉核心一个场所打开了哪些场所插件 |
| [`session.send` 的 `as`、`venue`](#sessionsend-的-asvenue) | 代表平台上的人说话 |
| [`session.respond`](#sessionrespond) | 根据旁听时记下的几条消息开始一轮 |
| [`session.note`](#sessionnote) | 只记下几块事实，不开始回合 |
| [`venue.records`](#venuerecords) | 判官查看的群聊记录 |
| [`events.append` 的 `venue.*`](#eventsappend-的-venue) | 记录撤回，记录 AI 发出去的话 |

**谁能调用**：`venue.sessions`、`venue.binding`、`venue.plugins`，以及关进沙盒的 `venue.session`（群、陌生人的私聊），只给平台桥调用（平台桥是核心拉起的、清单写了 `[connection] platform`、并且批准了 `act_for_external` 的扩展），其他调用者返回 `needs_approval`（`data.capabilities` 是 `["act_for_external"]`）。主人本人的私聊，主人这边的连接也可以打开。桥只能接触到它接入的平台的场所会话（按照场所编号的平台前缀判断）。代表平台上的人说话（带 `as` 的 `session.send`、`command.run`）、`session.note`、`venue.*`，以及记录 `venue.*` 事件，都需要能力 `act_for_external`。

**主人看场所会话**：关在沙盒里的场所会话（群、陌生人的私聊）只有接入这个平台的桥能往里面说话、修改它、删除它；主人这边的连接（人用的头，头是连接核心的界面程序，例如终端界面、网页界面）只能以只读的方式查看它：读取类的方法正常可用，写入类的方法返回 `session_not_found`，和没有这个会话一样：

- 读取类的方法：`subscribe`、`unsubscribe`（`events`、`view` 流，用 `after` 补发也正常可用）、`view.page`、`view.detail`、`session.prompts`、`session.tools`、`session.context`、`session.usage_parts`、`session.quota`、`job.output`、带 `session` 的 `blob.get`、`session.shown`。
- 其他带 `session` 的方法一律返回 `session_not_found`：说话、`command.run`、重做、撤销、打断、压缩、清空、更换配置、`set_*`、分叉、修改标题、删除等等。
- 扩展和平台桥不受这一条影响：桥仍然只能接触到它接入的平台的场所会话，读取也是一样。

**场所编号**：桥自己定的短名字（1 到 128 字节，不含控制字符），例如 `qq:private:10001`、`qq:group:123456`。核心不解读场所编号，只识别第一个 `:` 前面的平台前缀。

**主人对应表**：系统配置 `[external.bindings]`，写在这里的平台身份就是主人（因为只有一个主人，所以值写什么都算数，空的值不算）：

```toml
[external.bindings]
"qq:10001" = "owner"
```

在私聊里，这个号就是主人本人；在群里，这个号是主人。

语音助手那边的 `voice:owner` 不需要写进对应表，天然就是主人：它只在平台是 `voice` 的场所里算数，其他平台的桥报出这个号时，按照外人对待。

## `venue.session`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `venue` | 字符串，必写 | 场所编号 |
| `kind` | `private` 或 `group`，必写 | 私聊还是群 |
| `peer` | 字符串 | 私聊的对方；`private` 必须填写，`group` 不能填写 |
| `persona` | 字符串或 `null` | 创建会话时使用哪个人格（由桥按照自己的规则算好）；`null` 表示不使用人格 |
| `preset` | 字符串 | 创建会话时使用哪个预设 |
| `profile` | 布尔 | 创建会话时是否带上主人写的「关于你」：只有主人自己的私聊才填写 `true` |
| `images` | 布尔，默认 `true` | 这个场所能不能显示图片 |
| `voice_max_ms` | 正整数 | 这个平台上一段语音最长多少毫秒：只有语音有上限的平台才填写。这个值记进 `session.created`，这个会话的工具调用都会带上它（扩展的 `tool.call` 的 `voice_max_ms`），`say` 合成的语音超过这个长度时返回出错，让 AI 重新说。找回已有的会话时也会查看这个值：和记下的不一样时，另记一条 `session.policy_changed`（`voice_max_ms`），之后的调用带上新的值；不填写时沿用记下的值。填写 `0` 时返回 `bad_params`（`data.field` 是 `voice_max_ms`） |
| `titles` | 布尔，默认 `true` | 这个会话是否自动起标题：为 `false` 时从此不再自动起标题，人仍然可以手动修改标题。找回已有的会话时不查看这一项 |
| `cwd` | 字符串 | 创建会话时的工作目录；不填写时使用这个会话的默认工作区 |
| `recent` | 整数，1 到 1000 | 只用于群：开始一轮时最多带几条群聊近况（从最新的往前数），字节预算仍然作为上限，两者取较少的；不填写时只按照字节预算。创建会话时就固定下来 |
| `fresh` | 布尔，默认 `false` | 不找已有的会话，直接创建一个新的：最新的那个会话日志损坏（`session_read_only`）时，桥用这个参数另开一个；之后正常找回的就是新的会话，旧的仍然存在、看得到 |

上面用于创建会话的几项，找回已有的会话时不会查看。

**回应**：`{"session", "created"}`：会话编号，以及是不是新创建的。核心按照「场所加关不关沙盒」查找会话：如果私聊的对方后来写进了对应表，原来关在沙盒里的那个会话不再接着使用，而是另开一个不关在沙盒里的会话。

场所会话里没有人能确认：需要确认的操作一律拒绝。同一个 `id` 再发送一次时，返回同一个会话。

**出错**：`needs_approval`；`unknown_persona`、`unknown_preset`、`preset_invalid`；`bad_params`。

## `venue.sessions`

**参数**：没有。**回应**：`{"sessions": [{"session", "venue"}…]}`，从新到旧排列。列出的是这个桥接入的平台的场所会话（按照清单 `[connection] platform` 识别平台，这一项可以写几个平台），桥自己开的本机会话不列出。一个场所只列出最新的那一个会话。桥启动时按照这个列表订阅。

## `venue.binding`

**参数**：`id`：平台身份。**回应**：`{"owner": true|false}`：这个身份是不是主人（在不在主人对应表里）。核心按照这一刻的配置回答。桥拦截「只给主人」的操作时使用这个方法：对某个人动手之前，先识别目标的身份。

## `venue.plugins`

**参数**：`venue`：场所；`on`：打开的场所插件的名字（和 `provide` 的 `plugin` 相同），不填写或者为空表示一个都不给。**回应**：`{}`。

每次调用都整份替换。写了 `plugin` 的工具，在这个场所的会话里只有插件打开时才会提供，下一轮生效；这一轮里调用刚关掉的工具，返回「暂时不可用」。核心只在内存里记这份列表，重启以后所有场所都不提供：桥连上以后要对每个场所发送一次。桥只能管理自己平台的场所（按照清单 `[connection] platform` 判断，这一项可以写几个平台），其他平台的场所返回 `bad_params`（`data.field` 是 `venue`），名字写法不对时返回 `bad_params`（`data.field` 是 `on`）。

## `session.send` 的 `as`、`venue`

场所会话只接受带 `as` 的消息（没有带 `as` 时返回 `venue_session`），本机的会话填写了 `as` 时返回 `bad_params`。

**`as`**：`{"external": <平台身份>, "role": "manager" | "member"}`。`role` 默认是 `member`，斜杠命令根据它判断谁能使用；主人不靠 `role` 识别，而是按照对应表识别。

**`venue`**：一条平台消息的那几项，核心原样记下：

| 名字 | 说明 |
|---|---|
| `msg` | 平台的消息编号，必须填写 |
| `reply_to` | 引用的那一条消息的编号 |
| `name` | 发送者此刻在这里叫什么（群名片，没有时用昵称），最多 64 个字 |
| `my_name` | AI 此刻在这里叫什么（群名片，为空时用昵称；私聊里也用昵称），最多 64 个字。记忆根据它和人格的名字识别 AI：说的是这个名字的事，记成 AI 自己的事，不当成认识的人 |
| `place` | 这个群此刻叫什么（群名），最多 64 个字；私聊不填写。它只给记忆使用：在群里记下的记忆会带上它，AI 看到的话里没有它 |
| `mentions` | @ 了谁（平台身份的列表） |
| `mentions_me`、`mentions_all` | @ 了 AI；@ 了全体成员 |
| `media` | 消息带的东西 `[{"kind", "id", "name"?, "size"?, "shown"?}]`：`kind` 是 `image`、`file`、`voice`、`video`、`sticker`；`id` 是平台的编号（不会下载）；`shown` 表示这一样的原图已经作为附件交进来了 |
| `forwards` | 合并转发：`[{"nodes": [{"name", "text", "media"?, "forwards"?}], "more"?}]`，`media` 和上面相同，每一样还可以另外带 `index`（正整数，由桥给的编号，渲染成 `[image #3]`，按照这个编号来取；只能写在转发里，写在外面的 `media` 里时返回 `bad_params`），转发最多可以套 8 层；一个转发里的条数、整条消息合起来的条数、名字和正文的长度都没有另外的上限，只受一行最多 1 MiB 的限制（见[连接](connection.md)）。正文里要有占位（例如 `[forward]`） |
| `quoted` | 引用的那一条的内容（这一条引用了另一条时带上，群聊、私聊都接受）：`{"name", "text", "media"?, "forwards"?}`，和 `forwards` 里的一条完全一样，里面的转发最多 8 层；条数和长度与 `forwards` 一样，没有另外的上限。要同时带着 `reply_to`。带了这一项时，一律写在引用那一行下面（无论会话日志里有没有那一条）；没有带时只写编号。私聊里也写引用那一行，单独成一块 |
| `poke` | 戳一戳（QQ 的拍一拍、戳一戳：双击 AI 的头像），只能写 `true`（写 `false` 返回 `bad_params`）。桥把它当成戳的人发来的一条消息：`text` 可以是空的；AI 看到的是一句 `<名字> poked you (a QQ nudge: they double-clicked your avatar)`，平台名使用场所的平台，名字使用 `name`（没有时使用平台身份）。其他规则（旁听、判官、并行支线）和一条普通的消息相同 |
| `poke_text` | 戳一戳的后缀：平台上那个人自己设置的半句话（例如「的小脑袋」），转义以后接在那一句后面。只能跟着 `poke` 一起出现（单独写它返回 `bad_params`），不能是空白，最多 64 个字，不含控制字符 |
| `ambient` | 旁听：只记下，不开始回合 |
| `asleep` | 睡着时收到的消息 |
| `backfill` | 补拉回来的消息（桥断线时漏掉、之后从平台补回来的）：只进入聊天记录，不进入群聊近况和判官查看的记录；要同时带着 `ambient` |
| `sent` | 平台给的时刻，写法和事件的 `at` 相同（`2026-10-20T04:30:00.000Z`）；`history` 按照它显示。日志仍然按照记下的先后排列，序号不会往前挪 |
| `show_ids` | 渲染这一条时是否写出发送者的平台身份 |

**记成谁**：私聊里通过对应表识别出的主人本人，记成 `{"kind": "person", "via"}`；其他人记成 `{"kind": "external", "venue", "id", "role"}`，群里的主人另外带 `"owner": true`。

**命令编号**：桥按照「平台、登录的账号、消息编号、平台给的时刻」拼出 `id`，这样断线重发、平台重发都只生效一次。

## `session.respond`

这个方法根据旁听时记下的几条消息开始一轮（在群里判定要回复的时候）；不填写 `to` 时，根据几块事实开始一轮（例如到时间查看一下私聊）。

**参数**：`session`；`to`：可以不填写，是那几条 `message.user` 的序号，最多 64 条；`facts`：可以不填写，是几块事实 `[{"kind", "text"}]`（`text` 最多 4 KiB），排在触发消息的前面。`to` 和 `facts` 至少有一项不为空。不填写 `to` 时，事实先记下，由最后一块开始这一轮；扩展这样调用需要 `act_for_external`。`tail`：可以不填写，是一句文字（最多 2 KiB，空的当作没有填写）；开始的这一轮每次请求模型时，都会把它接在最末尾（在最新的消息、工具结果后面），它不进入历史、视图、导出和压缩，下一轮也没有它；适合每一轮都要带上的一句固定提示。

**回应**：`{"events": [...]}`。如果正在运行一轮，这几条会并进这一轮，AI 在下一步就能听到；带了 `tail` 时换掉这一轮原来的那一句，没有带时保持不变。

**出错**：`not_ambient`（`to` 里有不是旁听时记下的消息，另带 `data.messages`）；`already_answered`（已经作为触发用过的，另带 `data.messages`）；`bad_params`（`tail` 不是字符串或者超过 2 KiB 时，`data.field` 是 `tail`）。

## `session.note`

这个方法只记下几块事实，不开始回合、不打断，也不让空闲的 AI 接着运行（例如 AI 的话发不出去，下一步需要知道这件事）。

**参数**：`session`；`facts`：1 到 16 块 `{"kind", "text"}`。**回应**：`{"events": [...]}`。如果正在运行一轮，事实跟着这一轮，AI 在下一次请求时就能看到；如果是空闲的，AI 在下一轮开头看到。

## `venue.records`

这个方法返回判官（判断群里这一条要不要回复的那个小请求）查看的群聊记录，写法和 AI 看到的一行相同。

**参数**：`session`；`msg`：要判断的那一条的序号（带 `venue` 的 `message.user`）；`count`：1 到 100；`grow`：可选，起点对齐的步长，至少是 1，`count + grow` 不超过 100。填写了 `grow` 时，起点停在 `grow` 的倍数上（`(之前的条数 − count) / grow × grow`，小于 0 时从头开始），条数在 `count` 到 `count + grow − 1` 之间：这样起点在一段时间内不动，判官的请求前缀保持稳定，缓存就能命中。

**回应**：`{"records": "<之前的几条，一行一条>", "current": "<那一条>", "count": <之前的记录有几条>}`。模型回复过的话写成 `[you]` 行；撤回的消息同样会标出来。如果 `current` 引用了其他消息，编号后面会写出被引用的那一条的发送者和内容（`reply-to: msg=8805 小林: @Miyu 在吗`）；日志里没有那一条时只写编号；带了 `quoted` 时，在下面缩进写出它（日志里有那一条时两样都写）；之前的记录里的行也一样：带了 `quoted` 的会写出它。

**出错**：`bad_params`；`session_not_found`。

## `events.append` 的 `venue.*`

除了自己的 `ext.<前缀>.*`（见 [扩展](extensions.md#eventsappend)），桥还能往场所会话里记录两种事件：

| `kind` | `body` | 是什么 |
|---|---|---|
| `venue.recalled` | `{"msg", "by"}` | 有人撤回了一条消息：平台编号、撤回的人 |
| `venue.delivered` | `{"line", "turn", "to", "msg", "text", "images"?}` | AI 的话发出去了：哪个会话、哪一轮、回复给谁（平台身份的列表）、平台编号、正文、图片的哈希 |

这两种事件只接受场所会话里的，写法不对时返回 `bad_params`。之后 AI 看到的群聊里，撤回的消息会标出来，`[you]` 行是 AI 说过的话。
