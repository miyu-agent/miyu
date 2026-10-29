## 场所会话与外部身份（核心那一半）

### 是什么

通讯平台的桥（接入QQ，施工 S-6 下起在 `miyu-agent/miyu-onebot` 仓库）把平台上的人接进会话。核心不认识 QQ，只给几样通用的东西（`18-通讯平台.md` 第十六节 Q27）：

- **主人对应表** `external.bindings`：配置里一张平台身份的表，写了非空值的身份就是主人（值写什么都算，im 写 `owner`）。是不是主人，核心查它认，不信桥的一面之词。
- **场所会话**：一个场所（一个群、一个私聊）一个主线会话，桥用 `venue.session` 找回或者造。
- **代表外部的人说话**：`session.send` 多一格 `as`，桥说明这一句是平台上的谁说的、他在场所里是什么身份。代表别人只能降权：认主人是核心自己查表。
- **不在本机的头上**：场所会话不进 `session.list`、`sessions.changed`，跨会话的工具也不列它们（`18-通讯平台.md` 第十一节）。

状态：图纸，施工 O-3 做了（2026-10-07 主会话；形状给通讯平台的会话对过，对应表的写法、对应表改了怎么找回由项目主人同一天定）。由施工 O-3 做，斜杠命令施工 O-6 做了（2026-10-07 主会话；命令名、`/stop` 停到哪由项目主人同一天定）。`venue.binding` 施工 O-31 前、`venue.sessions` 施工 O-32 前做了（通讯平台的会话写、核心审）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-config/src/key.rs` | 键的占位多一种 `<external>`：平台身份，照短名字的写法 |
| `crates/miyu-endpoint/src/settings.rs` | `ExternalSettings`：`external.bindings.<external>`，项目配置不能写 |
| `crates/miyu-kernel/src/origin.rs` | `Person` 多 `via`（私聊里经哪个平台身份认出来的本人）；`External` 多 `owner`（对应表认出是主人的才写 `true`）、`role`（桥报的场所里的身份） |
| `crates/miyu-store/src/index.rs`、`index/row.rs` | 索引一行多 `venue`，版本 3 |
| `crates/miyu-endpoint/src/venues.rs` | `venue.session`：照场所加关不关找回或者造；`as` 怎么认、记成谁（照对应表和会话关不关）；`venue.binding`：问一个平台身份是不是主人（施工 O-31 前） |
| `crates/miyu-endpoint/src/venues/sessions.rs`（施工 O-32 前） | `venue.sessions`：列出桥接的平台的场所会话，每个场所一个 |
| `crates/miyu-endpoint/src/venues/plugins.rs`（施工 O-60） | `venue.plugins`：桥说一个场所开着哪些场所插件，整份替换，只管自己平台的场所 |
| `crates/miyu-session/src/plugins.rs`（施工 O-60） | `VenuePlugins`：每个场所开着的场所插件（内存里），工具面、快照怎么照它筛，回合里调到刚关掉的怎么判 |
| `crates/miyu-session/tests/venue_plugins.rs`、`crates/miyu-endpoint/tests/venue_plugins.rs`（施工 O-60） | 没说过的不给、开了下一轮给、关了下一轮没有、这一轮里刚关掉的再调回暂时不可用；没写 `plugin` 的照旧；本机会话不看；`plugin` 写错的 `bad_tool`；别的平台的、名字写错的 `bad_params`；本机的头 `needs_approval` |
| `crates/miyu-endpoint/src/venues/message.rs`、`crates/miyu-kernel/src/event/venue.rs` | `session.send` 的 `venue` 怎么查、记成什么（施工 O-13 上）；合并转发 `forwards` 的写法和上限（施工 O-34） |
| `crates/miyu-endpoint/src/appending.rs` | `events.append`：种类、大小、格怎么查，记成谁（施工 O-13 上） |
| `crates/miyu-endpoint/tests/venue_binding.rs`（施工 O-31 前） | `venue.binding`：桥问是主人的、没写的；写错的、多写格的、一次问几个的 `bad_params`；本机的头 `needs_approval`；只读 |
| `crates/miyu-endpoint/src/settings/tests.rs` | 主人对应表这一项在起来时就认 |
| `crates/miyu-endpoint/tests/platform_tools.rs`、`crates/miyu-config/src/package/links/tests.rs`（施工 IM-3） | 清单写两个平台：`tg:` 的群工具面有 `tg_send` 没有 `qq_send`、`qq:` 的群反过来，本机的会话都有；`qqnote`（没下划线）、`note` 哪里都给；两个平台的场所都能 `venue.plugins`，别的平台的 `bad_params`；主人第二个平台的私聊归主人、桥碰得到、列得出；软件包列表另带 `platforms`。列表空的、重复的、写法不对的、不是字的 `bad_platform` |
| `crates/miyu-endpoint/tests/venue_sessions.rs`（施工 O-32 前） | `venue.sessions`（两、三份核心先后起在同一个数据根上）：列得出、从新到旧；删了的、别的平台的、桥自己造的本机会话不列；从回收处拿回来的旧的、对方写进对应表另开以后关着的旧的不列；对应表里删掉了照列；多写格的 `bad_params`；本机的头 `needs_approval`（施工 SU-4 中改） |
| `crates/miyu-endpoint/src/venues/records.rs` | `venue.records`：照会话日志和快照里的时区渲染判官看的记录（施工 O-24） |
| `crates/miyu-endpoint/tests/venue_judge.rs` | `venue.records` 的写法、写错的、没有的会话（施工 O-24） |
| `crates/miyu-endpoint/src/responding.rs` | `session.respond`：参数怎么查，交给内核的 `Respond`（施工 O-14 上）；`session.note`：交给内核的 `Note`，事实的查法两边共用（施工 O-14 补） |
| `crates/miyu-endpoint/tests/respond.rs` | 照旁听的几条开一轮、`triggers` 排好去重、事实接在后面；写错的什么都不记；`not_ambient`、`already_answered` 带上是哪几条；同一个编号再发只算一次（施工 O-14 上） |
| `crates/miyu-endpoint/tests/note.rs` | 空闲时记下、不带回合编号、不开回合，下一轮的请求里排在触发前面；正在跑一轮时带回合编号；空的、太多的、写错的什么都不记；同一个编号再发只算一次（施工 O-14 补） |
| `crates/miyu-endpoint/tests/venue_records.rs` | `venue` 原样记下、旁听的不开回合、写错的什么都不记；`events.append` 收的三类、回应带序号、不带回合编号，拒的几种；扩展只能写自己的包那一段（`bridge_venues.rs`）（施工 O-13 上）；带的东西的 `size` 原样记，负的、小数、字、多写 `duration` 的拒（施工 O-33） |
| `crates/miyu-endpoint/tests/venue_forward_index.rs`（施工 O-61） | 转发里带的东西的 `index` 原样记、没写的不写；写在 `venue.media` 里的、0、负数、不是整数的 `bad_params`，什么都不记 |
| `crates/miyu-endpoint/tests/venue_forwards.rs`（施工 O-34） | `venue.forwards` 原样记下，只有转发、正文空的照旧 `empty_message`、什么都不记；套 4 层、一个转发 101 条、正文超过 4096 字节、名字超长或带换行或空白、空列表、`more` 写 0 或负的、空的 `nodes`、多写格、带的东西写错的 `bad_params`、不带 `data.limit`；整条消息 101 条、名字和正文超过 16 KiB 的 `bad_params`、`data.limit` 说是哪一样，正好 100 条、正好 16 KiB 的收；都什么都不记 |
| `crates/miyu-endpoint/tests/venue_shown.rs`（施工 O-35） | `venue.media` 的 `shown` 是真的原样记、假的不写；带的东西全是交了原图的、正文空、没有附件的 `empty_message`；转发里写了 `shown`、写的不是真假的 `bad_params`；都什么都不记 |
| `crates/miyu-endpoint/src/venues/bridge.rs`（施工 SU-4 中） | 平台桥照包认：核心拉起的、清单写了 `[connection] platform` 的；它接的平台 |
| `crates/miyu-endpoint/src/list.rs` | 列会话、推会话列表时跳过场所会话 |

### 对外的样子

**主人对应表**（配置，2026-10-07 项目主人定按编号的表）：

```toml
[external.bindings]
"qq:10001" = "owner"
"qq:10002" = "owner"   # 小号
```

| 键 | 类型 | 默认 | 层 | 项目配置 | 生效时机 |
|---|---|---|---|---|---|
| `external.bindings.<external>` | 名字（写什么都算主人） | 没有 | 系统 | 不能写 | `now`：下一句照新的认 |

- 键是平台身份，照短名字的写法（1 到 128 字节，没有控制字符），裸着写不下的照 TOML 加引号。主人有几个号写几行。
- 值写什么都算主人（im 写 `owner`），空的当没写。

**问对应表**（施工 O-31 前，2026-10-10 核心定；通讯平台的桥挡在场所里做的事要的，`providers.md`「在场所里做的事」）：`venue.binding {"id": <平台身份>}`，回应 `{"owner": <布尔>}`：这个号在主人对应表里（是主人）的是真。

1. 一次只问一个号；只读，对应表照旧。照这时的配置答（对应表改了，下一次问照新的）。
2. 只给平台桥（核心拉起的、清单写了 `[connection] platform` 的包的扩展，`venues/bridge.rs`）；别的连接照没批 `act_for_external` 回 `needs_approval`。
3. `id` 写错的（不合短名字的写法，例如空的）、多写格的、少写的回 `bad_params`。

**列场所会话**（施工 O-32 前，2026-10-11 和通讯平台的会话对过形状：桥起来就订阅，不等每个场所来一条消息才找）：`venue.sessions {}`，回应 `{"sessions": [{"session": <编号>, "venue": <场所>}, …]}`，从新到旧（照会话编号，就是照造的先后）。只给这两格。

1. 主会话里，每个场所只看最新的那一个（场所不是 `local` 的，删了的不算）；场所编号的平台前缀（第一个 `:` 前面那一段）在这个包清单 `[connection] platform` 里（施工 IM-3 起能写几个）的列。平台只认前缀，不算解读场所编号。对方后来写进对应表、另开了不关沙盒的那一个，这个场所最新的就是它；对应表里删掉了也照列（谁关沙盒等对方再来一句由 `venue.session` 照对应表算）。
2. 不分页：一个平台的场所是桥进过的群和私聊，桥起来时一次要全。
3. 只给平台桥，别的连接回 `needs_approval`（同 `venue.binding`）。多写格的回 `bad_params`。只读。

**`venue.session`**（命令）：找回或者造一个场所的主线会话。

| 参数 | 类型 | 说明 |
|---|---|---|
| `venue` | 字符串，必写 | 场所编号，短名字的写法，例如 `qq:private:10001`。核心不解读 |
| `kind` | 字符串，必写 | `private` 或 `group` |
| `peer` | 字符串 | 私聊的对方，平台身份，`private` 必写，`group` 写了是参数不对 |
| `cwd` | 字符串，可以不写 | 造的时候用；找回的不看 |
| `persona` | 字符串或 `null`，可以不写 | 造的时候用哪个人格（施工 P-1 上，`personas.md`）：桥照场所规则算好交来，核心不读场所规则；写 `null` 的明着无人格，不写的照默认人格，没设的无人格（施工 P-4 上：预设不再管默认人格）。找回的不看。明着写了没有的人格回 `unknown_persona`，什么都不造 |
| `preset` | 字符串，可以不写 | 造的时候用哪个预设（施工 P-2 上，`presets.md`）：同 `persona`，桥照场所规则算好交来；不写的照 `preset.default`。找回的不看。指着没有的回 `unknown_preset`、写错的回 `preset_invalid`，什么都不造 |
| `profile` | 布尔，不写是假 | 造的时候带不带你写的「关于你」（施工 P-9，`prompts.md`「关于你」）：桥照场所规则算好交来，主人本人的私聊这类才写 `true`；群聊不写。关起来的会话写了也不带。找回的不看 |
| `images` | 布尔，不写是 `true` | 这个场所显示得了图片吗（施工 X-25）：语音这类显示不了的写 `false`，清单写了 `needs = ["images"]` 的包在这个会话里整包当没开（工具不进工具面、回合开始不问、`system.append` 那一段不拼，也不列进「装了没开」那一行，`presets.md`「怎么走」）；记进 `session.created`，造的时候定、以后不变，子会话照父会话的。QQ 不写。找回的不看 |
| `titles` | 布尔，不写是 `true` | 这个会话自动起标题吗（施工 O-43）：机器人的会话这类不要标题的写 `false`，记进 `session.created`，这个会话从此不自动起标题（`kernel/session.md`「起标题」），人手改照旧改得了。找回的不看 |

回应：`{"session": <编号>, "created": <布尔>}`（桥要认陌生人照 `venue.binding`）。

1. 先定关不关：`private`、`peer` 在主人对应表里的是主人本人，不关，主人这边的连接（本机的头）也开得了；别的（群、陌生人的私聊）关进沙盒，只有平台桥开得了，别的连接回 `needs_approval`。关不关记进 `session.created` 的 `confined`，载入时照它关，不另存。
2. 照「场所加关不关」找：主会话里，`session.created` 的场所是它的、关不关一样的最新的那一个；删了的不算。对应表改了、关不关变了的，找不到就另造：旧的留着（2026-10-07 项目主人定）。
3. 找不到就造：场所照写，没人能确认（`attended` 是假：要确认的一律拒绝，工具面没有 `ask_user`），工作目录照 `cwd`，没写的、太宽的是主人的默认工作区。2026-10-08 定的「多收可选的 `workspace`」就是这一格，不另加。本机的头的会话列表照旧不列场所会话。
4. 同一个命令编号再发：交回上一次的那一个（和 `session.create` 一样）。去重照命令编号，不分连接：桥照「平台、登录的账号、消息编号、平台给的时刻」拼编号，不和头的撞。

**主人对应表认本人**：只在不关沙盒的会话里认，这个号说的话（`as`）记成主人本人（带 `via`）；关着的会话（群、陌生人的私聊）里主人说的是外部身份带 `"owner": true`（`venues/speaker`）。

**平台包不过预设**（施工 IM-4）：接平台的包在自己平台的场所会话里整包开着，预设关不掉它（`presets.md`「接平台的包在自己平台的场所会话里」）。平台自己的开关（场所插件 `venue.plugins`、平台后台的功能开关）照旧管。

**平台专用的工具**（施工 IM-3，接入通讯平台设计 07 第 4.6 节第 1 样）：接平台的包登记的、名字以 `<平台>_` 开头（平台是它清单 `[connection] platform` 里的一个）的工具是那个平台专用的（`miyu_tool::Tool::platform`，登记时定）。场所会话造工具面时只给场所平台对得上的：`tg:` 的群里不给 `qq_*`；本机的会话都给（终端里替人发消息）。不带平台前缀的照旧都给。工具面造会话时定，场所不变，不用每轮重筛。

**`session.send` 的 `as`**：

```json
{"session": "…", "text": "在吗", "as": {"external": "qq:10001", "role": "member"}}
```

| 格 | 说明 |
|---|---|
| `external` | 平台身份，必写 |
| `role` | 桥报的这个人在场所里的身份：`manager`、`member`，不写的是 `member`。斜杠命令照它判谁能用（O-6） |

- 主人不靠 `role` 认：照对应表认，记在外部身份的 `owner`、本人的 `via` 里。`role` 没有 `owner` 这一种，以后也不加：桥报的身份只能降权。

1. 只给场所会话：本机的会话写了 `as` 是参数不对。反过来，场所会话只收带 `as` 的：不带的回 `venue_session`（「这是通讯平台的场所会话，本机的头不能直接说话」）。主人定了群会话在终端里看不到、也不能打字（`18-通讯平台.md` 第十一节、Q16），只在列表里不列，拿着编号照样发话就是一条后门。桥发的每一句都带 `as`（主人的私聊也带，`external` 是主人的号），不挡桥。施工 Y-2 上起人的头连订阅都碰不到群会话（关在沙盒里的，见下面「不在本机的头上」）。
2. 记成谁（`message.user` 的 `by`）：
   - 不关的私聊、这个号在对应表里：本人，`{"kind":"person","via":"qq:10001"}`。
   - 别的：外部身份，`{"kind":"external","venue":<会话的场所>,"id":"qq:10001","role":"member"}`；这个号在对应表里的（群里的主人），另记 `"owner": true`：写的时候就记下，以后对应表改了，以前谁说的不跟着变（记忆照它认主人，`17-记忆.md` L16）。
3. 谁能写 `as`：本机连接都能（本人本来什么都能做，代表外部的人只会更低，施工 O-3）；核心拉起的扩展要声明 `act_for_external`，没声明的回 `needs_approval`（施工 X-30，`extensions.md`「谁能调」）。
4. 同一个命令编号再发只生效一次，核心重启以后照样（`04-核心协议.md` 第六节第 1 条）：桥照「平台、登录的账号、消息编号、平台给的时刻」拼编号（接入QQ 仓库的 `chat.md` 第七条第 1 条），断线重发、平台重发都靠它。

**场所的格**（施工 O-13 上，接入QQ 仓库的 `docs/blueprint/chat.md` 第七条第 2 条，2026-10-09 和通讯平台的会话又对过）：`session.send` 多一格可选的 `venue`，只跟着 `as` 来，原样记进 `message.user` 的 `venue`：

| 格 | 写法 | 是什么 |
|---|---|---|
| `msg` | 字，必写，1 到 128 个字符 | 平台的消息编号 |
| `reply_to` | 字，同上，可以不写 | 引用的那一条的平台编号 |
| `name` | 字，最多 64 个字符，可以不写 | 发的人此刻在这个场所里叫什么（群名片，没有的用昵称）；名字会变，每条各记各的 |
| `my_name` | 字，最多 64 个字符，可以不写 | 她此刻在这个场所里叫什么（群名片，空的用昵称；私聊也填昵称；施工 E-1 修三，2026-10-26 项目主人定由桥交、核心和记忆的会话定形状）；同 `name` 的规矩：去掉前后空白不空、没有控制字符，超长、写错 `bad_params`；名字会变，每条各记各的，原样记进 `message.user` 的 `venue`。记忆认她的时候和人格的名字一起算（`memory.md` 第六条第 6 款）；群里那一行、判官看的记录都不用它 |
| `place` | 字，最多 64 个字符，可以不写 | 这个场所此刻叫什么（群名；私聊不写，核心不查是群还是私聊；施工 E-4 上，2026-10-27 项目主人定「她看得到每条记忆在哪儿发生的」、核心和记忆的会话定形状）；同 `name` 的规矩：去掉前后空白不空、没有控制字符，超长、写错 `bad_params`；群名会变，每条各记各的，原样记进 `message.user` 的 `venue`。只给记忆：记下时存进 `ext.memory.saved` 的 `place`（`memory.md`「对外的样子」）；渲染她上下文的地方（群里那一行、群聊近况、判官看的记录）都不读它，请求一个字节都不因它变 |
| `mentions` | 平台身份的列表，可以不写 | @ 了谁；@ 了谁的名字桥写在正文里 |
| `mentions_me`、`mentions_all` | 布尔，不写是假 | @ 了她；@ 了全体成员（不算 @ 她） |
| `media` | 列表，可以不写 | 带的东西，每项 `{kind, id, name?, size?}`：`kind` 是 `image`、`file`、`voice`、`video`、`sticker`，`id` 平台的编号（懒下载，不进内容块），`name` 文件名、表情的字（最多 200 个字符），`size` 多少字节（非负整数，平台说了的才写；施工 O-33，群里的一行照它写大小，`kernel/request.md`「群里的一行」），`shown` 真假（可以不写，不写是假；施工 O-35，2026-10-11 核心审过）：这一样的原图已经当附件交进来了，O-35 起造的私聊会话写记号时跳过它、编号照旧连着数；它不算内容（全是交了原图的、正文空、没有附件的 `empty_message`）。假的不记，以前记的没有，字节不变。转发里的 `media` 不能写 `shown`（写了 `bad_params`）；`index` 正好反过来，只在转发里有（下面 `forwards`），写在这里的 `bad_params`；群里的一行这一步不看它（桥只给私聊填）。不收时长：平台的段里没有（2026-10-11 主会话、核心定，不为以后写） |
| `forwards` | 列表，可以不写 | 合并转发的内容（施工 O-34，2026-10-11 核心审形状时定）：照正文里转发的先后，每项 `{nodes, more?}`。`nodes` 1 到 100 条，每条 `{name, text, media?, forwards?}`：`name` 转发里那个人当时的名字（同上面的 `name`：去掉前后空白不空，最多 64 个字符，没有控制字符），`text` 那一条的正文（最多 4096 字节，可以是空的，换行照收、渲染时转义），`media` 同上面的 `media`，每项另可以写 `index`（施工 O-61，2026-10-29 主会话和桥对过定）：正整数，桥照 `fetch_media` 的数法给的编号（同一条消息里接着它自己带的东西往下数，照展开的先后，套着的转发也算），写了的记号标 `#编号`、照它取，没写的只写记号；0、负数、不是整数的 `bad_params`；核心只照记的写、不自己数，不查重（重了是桥的事）；以前记的没有，字节不变。`fetch_media` 怎么取由桥定、说明在桥的包里，`forwards` 套在里面的转发（同样的形状）；`more` 桥没放进来的条数，只在大于 0 时写。套的层数最多 3（`venue.forwards` 是第一层）。整条消息另有上限：所有转发（连套在里面的）合起来最多 100 条、`name` 和 `text` 合起来最多 16 KiB，超了的 `bad_params`，`data.limit` 是 `forward_nodes` 或 `forward_bytes`；单层的不合不带 `data.limit`。写了空列表的、`more` 写 0 的 `bad_params`（没有转发就不写这一格）。核心不拿正文里 `[forward]` 的个数对转发的条数（人也打得出这几个字）。转发不算内容：带转发的消息正文里要有占位（桥写 `[forward]`），只有转发、没有字也没带东西的照旧 `empty_message`（2026-10-11 核心审时定：O-34 以前造的私聊会话不写转发那几行，这样一条渲染出来是没有块的 user）。怎么渲染见 `kernel/request.md`「合并转发」 |
| `ambient` | 布尔，不写是假 | 旁听：只记下，不开回合，回合进行中也不排进这一轮 |
| `asleep` | 布尔，不写是假 | 睡着时收到的（桥照样带 `ambient`）；群聊近况不收它（O-13 下） |
| `backfill` | 布尔，不写是假 | 补拉回来的（施工 O-40，设计 18 第十一节）：桥断线时漏掉、重连以后从平台补回来的，只进聊天记录（`history` 搜得到）。要带着 `ambient`，不带的 `bad_params`；同睡着时收到的，群聊近况、判官看的记录都不收它 |
| `sent` | 时刻，可以不写 | 平台给的时刻（施工 O-40），写法同事件的 `at`（`2026-10-20T04:30:00.000Z`），不合的 `bad_params`。日志照记下的先后追加，序号不因它往前挪；`history` 显示、照时刻筛都照它 |
| `show_ids` | 布尔，不写是假 | 渲染这一条时写不写发的人的平台身份（施工 O-13 中）：桥照这时的场所规则每条带上，规则改了从下一条起照新的 |

写错的、不带 `as` 的：`bad_params`，什么都不记。

**判官看的群聊记录**（施工 O-24，接入QQ 仓库的 chat.md 第六条 `Ask.records`、`Ask.current`，形状 2026-10-09 和通讯平台的会话对过）：`venue.records {session, msg, count, grow?}`，回应 `{"records": "…", "current": "…", "count": N}`（`grow`、`count` 施工 O-44）。

1. `msg` 是要判的那一条，必须是这个会话里带 `venue` 的 `message.user`；`count` 1 到 100。写错的、`msg` 不对的 `bad_params`；没有这个会话的 `session_not_found`。
   - `grow`（施工 O-44，接入通讯平台 2026-10-21 要的）：可选，至少 1，`count + grow` 不过 100，不合的 `bad_params`。写了的，之前的记录起点照步长对齐：一共 n 条时从第 `max(0, (n − count) / grow × grow)` 条起（整除），条数在 `count` 到 `count + grow − 1` 之间。来了新消息起点不动，满一步才往前挪，判官的请求前缀一段时间不变，缓存命中得上。没写的照旧是最近的 `count` 条。回应的 `count` 是之前的记录实际有几条（`miyu_assemble::group::Window`）。
2. `current` 是那一条的一行；`records` 是它之前的群里的话，一行一条（每行以换行结尾，没有的是空的），照日志的先后，从新往旧取 `count` 条：场所的 `message.user`（旁听的、开过回合的都收，睡着时收到的、补拉回来的不收），`venue.delivered` 写成 `[you]` 行（判官看不到她的回复，主线的也收）；`msg` 以前记下的撤回照样标。
3. 写法和她看到的一行同一个函数（`miyu_assemble::group::records`），照会话快照里钉下的时区和字；快照里没有的（私聊）照核心这时的时区和出厂的字。合并转发那几行（施工 O-34）同样写在那一条下面：群会话照快照的 `core.forward`，O-34 以前造的没有就不写（和她看到的一样）；快照里没有群会话的字的照这时的。照会话日志读（同 `view.page`，不载入会话），撤掉的、撤回的不算，压缩换出去的照样算。
4. 引用（施工 O-41，照旧版判官看的，2026-10-21 和通讯平台的会话对过）：`current` 那一条引用了别的，`reply-to: msg=编号` 后面空一格写引用的那条的发的人和内容，照日志里那一行的写法（名字、`id=`、身份同那一行；内容同一行正文的截法、转义，带的东西照记号；她发的写 `[you]`）：`reply-to: msg=8805 小林: @Miyu 在吗`。认 `msg` 以前日志里最后一条这个编号的，睡着时收到的、补拉回来的也认；日志里没有的照旧只写编号，不另加一句。`records` 里的行、她看到的一行照旧只写编号（前缀不变）。群名核心不记，不写。
5. 判官带人格（2026-10-09 项目主人定，出厂开，能按场所关）：桥照订阅回应里的 `persona` 用 `persona.read {persona, prompt: "persona"}` 读现在文件里的那一份，开关是群聊内核的出厂参数 `judge.persona`（通讯平台的会话 O-23 补）；`venue.records` 不管人格，形状不变。

**照记下的几条开一轮**（施工 O-14 上，接入QQ 仓库的 chat.md 第七条第 3 条第 1 项，形状 2026-10-09 和通讯平台的会话对过）：`session.respond {session, to, facts}`，回应 `{"events": [...]}`（`turn.started` 和事实的序号，同 `session.send`）。

1. `to`：序号的列表，1 到 64 条，核心照序号排好、去重；`facts`：可以不写，每块 `{kind, text}`，`kind` 照事实类别的写法，`text` 最多 4 KiB，原样记成 `context.injected`，排在触发前面。写错的 `bad_params`，什么都不记。
2. 记成谁同下面的 `events.append`。同一个命令编号再发只算一次，回应和头一次一样。
3. 正在跑一轮的并进这一轮（施工 O-14 下）：事实和一条 `turn.joined {triggers}`（带回合编号）当场记下，下一步就听到，这一轮没再请求就结束的接着开一轮，打断时不退回、不接着开。`to` 里有不是这个会话里旁听的 `message.user` 的 `not_ambient`，有已经当过触发的 `already_answered`，`data.messages` 是不合的那几条。
4. 开的那一轮 `turn.started` 带 `triggers`，那几条在回合开始的地方渲染（群会话里一行一条），以后的群聊近况不再收它们。

**记几块事实**（施工 O-14 补，2026-10-09 和通讯平台的会话对过，它的退信要的：她的话发不出去，下一步知道）：`session.note {session, facts}`，回应 `{"events": [...]}`（事实的序号）。

1. `facts` 1 到 16 块，写法同上面的 `facts`，原样记成 `context.injected`。写错的 `bad_params`，什么都不记。记成谁同下面的 `events.append`；同一个命令编号再发只算一次。哪个会话都收。
2. 不开回合、不打断、不叫醒。正在跑一轮的带这一轮的回合编号，下一次请求就看到，撤销这一轮跟着撤；这一轮没再请求就结束的，下一轮开头看到。空闲的不带回合编号，下一轮开头看到，排在那一轮的触发前面。
3. 不并进 `events.append`：那条路记的一律不带回合编号，在一轮里来的退信该跟着这一轮。

**桥记的事件**（施工 O-13 上，接入QQ 仓库的 chat.md 第七条第 3 条第 2 项）：`events.append {session, kind, body}`，回应 `{"seq": n}`。记成不带回合编号的事件，任何时候都收，不开回合、不打断；记成谁：核心拉起的扩展是那个包（模块），本机的头是主人。

1. `ext.<包>.<名字>`：`<名字>` 一段或几段、点隔开，每段小写字母开头，只有小写字母、数字、`_`、`-`；整个种类最多 128 个字符；`body` 是 JSON 对象，序列化以后最多 16 KiB。核心拉起的扩展的连接，`<包>` 必须是它自己的包编号；本机的头都收。哪个会话都收。不渲染，撤销、压缩都不动它。
2. `venue.recalled`：`{msg, by}`，被撤的平台编号、谁撤的平台身份。
3. `venue.delivered`：`{line, turn, to, msg, text, images?}`，哪条线的会话编号、哪一轮的回合编号、回的人（平台身份的列表）、平台编号、正文、图的内容哈希。
4. 第 2、3 条只收场所会话的；格多了、少了、写法不对的，和第 1 条不合的，都 `bad_params`，什么都不记。O-13 下照它们渲染撤回的标记和 `[you]` 行。

**场所插件**（施工 O-60，2026-10-24 项目主人定、核心主会话转来：禁言、踢人这类管理工具只在群主开了的群里给）：`provide` 的工具可以写 `plugin: <名字>`（`extensions.md`「`provide`」），名字写法同包的编号。

1. 场所会话（场所不是 `local` 的）里，写了 `plugin` 的工具只有那个场所开着这个插件才进工具面；没说过的场所一件都不给，核心、桥重启以后也是。本机会话不看这个标记。同一个包里没写 `plugin` 的照旧。
2. 桥说哪些开着：`venue.plugins {"venue": <场所>, "on": [<名字>, …]}`，回应 `{}`。整份替换，空的（或不写 `on`）是全不给。只在核心的内存里记，不落盘：桥连上以后对每个场所发一次。
3. 下一个回合开始照它重筛工具面，开、关都破一次前缀缓存。快照的 `plugin_tools` 记下给了的那几件（`policy.md`）。
4. 这一轮里刚关掉的，模型再调回「暂时不可用」（`unavailable`），工具不跑；下一轮工具面上已经没有它，再调是内核的「不在工具面上」。
5. 只给平台桥，别的回 `needs_approval`（施工 SU-4 中）。场所编号的平台前缀要在这个包清单 `[connection] platform` 里（施工 IM-3 起能写几个），别的平台的、清单没写平台的回 `bad_params`（`field` 是 `venue`）；名字写法不对的 `bad_params`（`field` 是 `on`）；场所编号写错的、多写格的 `bad_params`。

**斜杠命令**（施工 O-6，`protocol.md` 的 `command.run`）：桥把人打的原文连同 `as` 交过来，核心认、判谁能用、执行，回执那一句也由核心照语言写好，桥原样发出去。

1. 头一批两个：`/clear`（别名 `/reset`）清空她看到的上下文；`/stop` 全停：打断这一轮，排着的话留在聊天记录里、不回、不撤回，再停掉她派出去的后台命令和子代理。终端、网页、通讯平台同一套名字。施工 9-7 下加 `/workspace <路径>`：换会话在哪个目录干活（`protocol.md` 的 `command.run` 第 5 条）。
2. 谁能用：私聊里对应表认出的本人、群里对应表里有的外部身份（带 `owner` 的）、`role` 是 `manager` 的；别人 `command_not_allowed`。本机的会话本来就是本人。`/workspace` 动的是沙盒能写的地方，只有本人（私聊里认出的、群里带 `owner` 的）能用，管理的人 `owner_only`。
3. 执行了的记一条 `command.ran`（原文、正名），聊天记录看得到谁在什么时候清过、停过；被拒的什么都不记。命令编号照 `session.send` 的拼法，同一个只生效一次。

**不在本机的头上**：`session.list`、`subscribe` 的 `sessions`、推送 `sessions.changed`、她的 `sessions` 工具、`history` 读别的会话，都跳过场所不是本机的会话（场所是 `local` 的才算本机的）。施工 Y-2 上起（2026-10-23 项目主人定：场所会话只读、不从头上交互，看、删放在平台自己的后台）拿编号也碰不到：带 `session` 的方法办之前统一查碰不碰得到（`protocol.md`「关着的场所会话只给桥」），关在沙盒里的场所会话（群、陌生人的私聊），人的头订阅、发话、删都回 `session_not_found`，只有接这个平台的桥碰得到；主人本人的私聊不关，头和桥都碰得到（`connection/checks.rs`）。删场所会话归桥（`session.delete` 对扩展要 `sessions.drive`），平台的后台照它给主人删。

### 起草时定的

- 对应表按编号的表，不是表的列表：配置系统改得少（2026-10-07 项目主人定）。
- 关不关变了另造：旧会话里的话是在外部的信任下说的，整个转给不关的会话，等于让它继承主人的权限。
- 核心不解析场所编号：是不是私聊、对方是谁，桥照实报；核心只在私聊时拿对方查对应表，群不查。`kind` 所以由桥报。
- 场所会话不收不带 `as` 的话：回专门的原因码 `venue_session`，不混在参数不对里，头看得出是哪一种拒绝（2026-10-07 通讯平台的会话提、主会话定）。
- `role` 记进 `by`：由桥担保，和平台身份同一个担保。
- 斜杠命令单开一个方法 `command.run`，不混在 `session.send` 里：头不用自己认命令，认不出的核心回 `unknown_command`，桥照原话回人（2026-10-07 项目主人定单开一步）。清空的命令叫 `/clear`、别名 `/reset`；`/stop` 是全停，排着的留在历史里不回（同一天项目主人定）。
- 群、陌生人私聊的会话关进沙盒（照对应表认对方是不是主人）：主人在群里说的是外部身份带 `owner`，不是本人。桥照包认（核心拉起的、清单写了平台的），没批 `act_for_external` 的 `needs_approval`；桥认陌生人照 `venue.binding`。
