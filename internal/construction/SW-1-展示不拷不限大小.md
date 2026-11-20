## 施工单 SW-1：展示不拷进核心、不限大小

状态：做完，等主会话审、合（2026-11-12 写；项目主人同一天定：`show` 以后没有大小限制，本机和局域网都直接展示，核心不再拷一份）。

### 为什么

项目主人 2026-11-12 让她展示一段录好的视频，被 20 MiB 的上限挡住（`show/too-big.txt`）。上限是施工 D-5 照附件定的：展示时核心把整份读进内存、存成 blob，头照 `blob.get` 读。视频、录屏动辄几百 MiB，拷一份既占数据根、又要整份读进内存。

### 改了什么

1. **不拷**：`show` 只打开文件、取大小和修改时间（照打开的那个句柄取，不另 stat 路径）、读开头至多 1 MiB 认种类，不存 blob。20 MiB 上限、`show/too-big.txt` 和三份给人看的 `show/too-big` 一起删掉。路径照旧照 `read` 的规矩换成真实位置、权限照它判、打开不跟最后一层链接。
2. **`file.shown` 换形状**：去掉 `blob`，多 `modified`（修改时间，UTC 毫秒，写法同事件的 `at`）。`size` 是展示那一刻的字节数。以前记的带 `blob`、没有 `modified` 的照常读得进（`blob` 不认、不写出去），不另写分支。
3. **认种类只看开头**：图片、PDF、音视频本来只看文件头；「是不是 UTF-8」改成只看读到的那一截（至多 1 MiB），截断处半个字不算坏。1 MiB 以内的文件结果和以前一样。
4. **本机的头照路径读**：握手回应带 `host` 的连接是本机的（同 AT-1 的判法，`connection/reach.rs`），直接照 `shown.path` 读硬盘。核心不替它读。
5. **远程的头分段读**：新方法 `session.shown {session, call, offset?, length?}`，回应同 `blob.get`（`{data, size}`，一段最多 512 KiB，写 0 只问大小）。`call` 是视图工具卡的 `call`。核心从调用编号里的助手消息序号读起（`read_range_only`，只解 `tool.result`），找到那次调用的结果、取它的 `file.shown`，不整份翻日志：视频拖进度一次次来读也不慢。
6. **错误码**：没展示过的、会话没有或读不到的一律 `unknown_shown`（同 `unknown_blob`，不分说法）；打开时大小或修改时间和记的对不上 `file_changed`；没了、不是普通文件了 `file_gone`；别的读不了 `internal_error` 记一行。参数里多写的格（例如 `path`）`bad_params`。

### 安全的理由

- 核心只读日志里 `file.shown` 记下的那个路径：参数里没有路径，写了别的格当场拒。路径是她展示时照权限策略判过、换成真实位置的。
- 读之前照打开的句柄核对大小和修改时间：展示以后被换成别的文件（改了、换了链接指向）就拒，不会借这个口读到展示以外的内容。打开照旧不跟最后一层链接。
- 谁能读：照 `blob.get` 带 `session` 的那一套（`attach/reach.rs` 的 `reachable`）：人的连接读得到本机会话、不关沙盒的场所会话；平台桥只读得到它那个平台的场所会话；扩展要那个会话是它看得到的。这个文件就是那个会话展示过的，不用另查引用。
- `session.shown` 不走「关着的场所会话只给桥」那一道（同 `blob.get`），拒的一律 `unknown_shown`，免得拿两种说法探会话编号。

### 头要做什么

- 网页、终端、im 照 `shown` 新的形状画：没有 `blob` 了。
- 本机的头（握手回应有 `host`）照 `shown.path` 读；远程的头用 `session.shown` 分段读：视频、音频边传边放（拖进度照 HTTP Range 那样按段取），很大的文本只取开头预览。
- 读到 `file_changed` 显示「文件已变」，`file_gone` 显示「文件已不在」。
- im 发展示的文件进群：从 `blob.get` 改成 `session.shown`（桥读得到它平台的场所会话）。
- 终端「当场开一次」直接开 `shown.path`，不用再取到临时目录。

### 蓝图、参考改了哪几处

- `tools/show.md`：是什么、在哪、怎么走第 2、3、4、5 条、效果那张表、出错、守着它的。
- `kernel/events-bodies.md`：`file.shown` 那一行。
- `view.md`：`tool` 的 `shown`。
- `protocol.md`：方法表、`blob.get` 第 1 条去掉 `file.shown` 的引用、新加 `session.shown`、错误码表、给人看的字、守着它的、在哪。
- `prompts.md`、`26-提示词.md` 登记簿：去掉 `show/too-big.txt`。
- 设计 `35-展示.md`：第一节第 4 条、第二节第 1、2、4 条改成现在的样子，「定了的」加一行。
- 参考 `blob`、`view`、`errors` 中英，`methods-v1.txt` 加 `session.shown`。

### 不做什么

- 不做旧版兼容：以前存的 blob 不再经 `blob.get` 带会话读得到；以前的 `file.shown` 远程读回 `file_changed`（没有 `modified` 可核对）。没发版。
- 核心不替头截：预览多少、怎么边传边放是头的事。
- 撤掉的那一轮里展示过的照样读得到：头不再画它，读法不另判（同 `blob.get` 照会话找 blob 不看撤没撤）。

### 怎么验

- 测试先写、改前红：`miyu-basesystem` 的 `show::a_file_over_twenty_mib_is_shown`（改前回 too-big）；`miyu-endpoint` 的 `session_shown` 三个（改前 `show` 被挡、`session.shown` 不认识）。
- `miyu-basesystem` `tests/show.rs`：稀疏的 20 MiB 多一个字节的文件照常展示，效果记路径、大小、修改时间；`show/kind/tests.rs`：截断处半个字照文本认。
- `miyu-endpoint` `tests/session_shown.rs`：日志里的 `file.shown` 没有 `blob`、存处没多东西；远程的连接读开头、读到结尾就停、过了结尾是空的、写 0 只问大小、不写读 512 KiB、超过 512 KiB `bad_params`；本机的也读得到；读的调用、没有的调用、没有的会话 `unknown_shown`，写法不对、多写 `path` 的 `bad_params`；只改修改时间、只改大小 `file_changed`，改回去又读得到，删了 `file_gone`。
- `miyu-session` 效果的测试、`miyu-view` 工具卡的测试换成新的形状；以前带 `blob` 的 `file.shown` 照样读得进。
