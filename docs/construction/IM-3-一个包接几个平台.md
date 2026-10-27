## IM-3：一个包接几个平台

状态：主会话做，一步，2026-10-28 做完。接入通讯平台（im）设计 07 第 4.6 节第 1 样（核心编号「核心-甲」）。

### 一、为什么

im 要在一个包里接 QQ 和 Telegram（im 设计 07 第 4.3 节）。现在清单 `[connection] platform` 只能写一个字。场所归属、场所插件这些按平台放行的地方都只认这一个。工具也不分平台：`tg:` 的群里会给 `qq_*`。

### 二、定下的（技术题，主会话定）

1. **清单**：`platform` 写一个字照旧；写列表的是几个，照写的先后，不能空、不能重复，每个写法同包的编号；不对的报 `bad_platform`。读进来一律是列表（`Connection::platforms`）。
2. **按平台放行的地方都改成「在列表里」**：`venue.sessions` 列谁的（`Owners`，对应表照（平台，账号）记），`venue.plugins` 管哪些场所，桥碰得到哪些会话（`bridged`）。
3. **工具照场所的平台放**：
   - 接平台的包登记的工具，名字以 `<平台>_` 开头、平台是它清单里的一个，是那个平台专用的。
   - 这个记号在工具上：`Tool::platform`，登记时定，同 `venues`。
   - 场所会话造工具面时只给平台对得上的；本机的会话都给（终端里替人发消息，施工 I-40）。没有平台前缀的照旧都给，前缀后面没有下划线的（`qqnote`）不算。
   - 工具面造会话时定，场所不变，用不着每轮重筛。
   - 现在已有的 QQ 会话、本机会话字节不变。
4. **列表里怎么报**：`package.list` 的 `connection` 照旧带 `platform`，取第一个；接几个平台的另带 `platforms`（全部）。装之前的预览同理：`connection` 照旧是第一个，另带 `platforms`。只接一个平台的一字不变，两个头不用改。
5. 场所编号多一段机器人号（im 设计 07 第 4.2 节）核心不用改：平台照旧只认第一个 `:` 前面那段。

### 三、做了什么

1. `miyu-config` 的 `links.rs`：`Connection { platforms }`、`has`；读一个字或者列表。
2. `venues/sessions.rs`：`Owners.platforms` 是集合，`bound` 记（平台，账号）；`platforms_of_package` 挪成公共的，`bridged`、`platform_is`、`take` 照列表判。
3. `miyu-tool`：`Tool::platform`，默认没有。`provide/remote.rs` 的 `RemoteTool::for_platform` 照名字前缀和包的平台定；`provide.rs` 登记时带上（登记缓存走同一条路）。
4. `miyu-session` 的 `agents.rs`：场所会话的工具面拿掉平台对不上的。
5. `packages/fill.rs`、`packages/preview.rs`：接几个平台的另带 `platforms`。
6. 作者说明 `manifest.md` 中英两份、蓝图 `packages.md`、`venues.md`（「平台专用的工具」），协议参考 `venues.md` 中英两份。
7. 测试夹具：照对应表起核心、等扩展记下回应的两个挪进 `tests/support/served.rs`（`system_account.rs` 到了 500 行），多一个照给的系统配置起的 `served_core_with`。

### 四、怎么验

1. `miyu-config` 单测：写列表的照先后读进来，`has` 认得出；空的、重复的、有一个写法不对的、不是字的、写成数的都是 `bad_platform`。原来一个字的写法照过。
2. 端点 `platform_tools.rs`：桥清单写 `["qq", "tg"]`，登记 `qq_send`、`tg_send`、`note`、`qqnote`。
   - `tg:` 的群：`note`、`qqnote`、`tg_send`。
   - `qq:` 的群：`note`、`qq_send`、`qqnote`。
   - 本机的会话：四件都有。
   - `venue.plugins` 对 `tg:` 的场所照收，对 `discord:` 的回 `bad_params`（`venue`）。
   - 软件包列表的 `connection` 是 `{"platform": "qq", "platforms": ["qq", "tg"]}`。
   - 主人在第二个平台的号（对应表另写 `tg:777`）：私聊归主人，桥在里面替他说得上话，`venue.sessions` 列得出。
3. 原有场所、系统账号、每人一份、软件包列表的测试照过（一个平台的 `connection` 一字不变）。
4. 手写变异 8 个全杀：不认平台前缀、本机会话也筛、没下划线的也算、几处「在列表里」只认第一个、列表能重复、列表能空。第二个平台的对应表、桥碰主人第二个平台的私聊这两条头一轮活了（测试里只有 QQ 的对应表），补了 `tg:777` 那一段才杀掉。
5. 本机门禁全过。
