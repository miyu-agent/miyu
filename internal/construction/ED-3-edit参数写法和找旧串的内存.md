## 施工单 ED-3：edit 认的每一种参数写法都一样对，找旧串不再造逐字节的对照表

状态：做完，等验收（分支 `step/ED-3-edit-参数和内存`）。2026-11-27 core1 派：LiteMiyu 实测出 `edit` 的两处问题（它那一套是从这里搬过去的写法），这里核实都有。

### 目的

1. `edit` 认的几种写法（顶层直接写一处、`edits` 写成一个对象、`path`、`filePath`、`oldText`、`replaceAll` 这些别名），照参数格式修正写成字符串的值、视图的标题和估的行数、命令行和 ACP 取路径和改动，都和标准写法一样对。复现：顶层写一处加 `"replace_all":"true"`，报 `invalid type: string "true", expected a boolean`（小模型 DeepSeek flash 连试三次都这样）；别名写的成了，视图里标题没有路径、没有估的 `+N -N`，头照参数画的差异和文件修改也没有。
2. `edit` 找旧串时不再给整份原文造逐字节的对照表。复现：改 500 kB 的文件一处，峰值常驻内存多占 9.0 MB（精确对上）、8.1 MB（宽松对上）。

### 根因

- 内核照参数格式修正（`tool::repair`）只走参数格式里声明了的位置，声明的是标准写法：顶层的 `replace_all`、`edits` 是对象时里面的、`replaceAll` 都不在声明里，写成字符串的不还原，工具的 serde 读到字符串就报错。视图（`estimate.rs`、`title.rs`）、命令行的编辑那一块、ACP 的位置也只照标准写法的 `file_path`、`edits` 读。
- `find.rs` 的 `View` 给看法里的每个字节记一对 `(usize, usize)` 来处：原文 1 字节就是 16 字节的表；精确的那一层也造，用不着找的那一层也造。

### 蓝图改哪几节

- `kernel/tools.md`：「在哪」加 `tool/shape.rs`；新加「标准写法」一段。
- `tools/edit.md`：「在哪」「参数格式里不写、写了也认的」「怎么走」第 1 条、「找位置」第 4 条、「守着它的」。
- `view.md`：条目 `tool` 加 `standard_args`；「守着它的」加 `tests/shapes.rs`。协议参考 `docs/{zh,en}/reference/view.md` 同步加 `standard_args`。
- `cli/ask.md`「编辑那一块」第 2 条和测试表、`cli/acp.md`「`locations`」和测试表。

### 不做什么

- 不改内核的修正、不改落盘的内容、不改发给模型的请求：旧日志里不是标准写法的调用原样留着，用的时候才规整。
- `write`、`read`、`trash`、`show` 的执行不动：它们只有路径的别名，serde 本来就认。
- 终端界面、网页照参数算差异的代码不在这个仓库：写清在哪、怎么改，交 core1 转给网页、终端的会话。
- `Style::of` 为了认换行整份解一遍、`edit` 解码另抄一份这两处（LiteMiyu 一起改了）不做：`edit` 要留着原来的字节报 `file.changed`，借不走；那一份是一下就放掉的，不在峰值上。

### 验收

- 改代码之前红、改了以后绿：`crates/miyu-basesystem/tests/edit_shapes.rs` 的 `every_shape_takes_a_boolean_written_as_text`（改前报 `invalid type: string "true", expected a boolean at line 1 column 75.`）；`crates/miyu-view/tests/shapes.rs` 的两条（改前标题的对象是 `null`、没有估的行数）；`crates/miyu-cli` 的 `an_edit_in_another_shape_has_its_path_and_block`、`a_path_alias_still_names_the_location`；`crates/miyu-basesystem/tests/edit_memory.rs`（改前精确 9068544、宽松 8065024 字节，预算 4000000；改后 1503232，先量宽松的是 1568768）。
- `edit_shapes.rs` 的 `what_cannot_be_read_says_what_it_said_before`：`"yes"`、写了两个名字、缺 `file_path` 的报错和改前逐字一样（照改前跑出来的抄的）。
- `find/tests_cross.rs`：原来逐字节的表照抄在测试里，随机文件三层看法对拍（平时 400 份，长跑 2 万份）。
- 请求形状探针（`miyu-assemble` 的 `probe*`）照旧全过：发给模型的字节没变。本机门禁全过、三平台 CI 绿。

### 风险

- 规整出来的和工具原来读出来的不一样：规整只换名字、挪位置，读之前还是同一个 `Args`；读不出来就照原文读，报原文的错。`shape/tests.rs` 每一种写法、`edit_shapes.rs` 走真工具。
- 视图多一格 `standard_args`：只有不是标准写法时才写，翻页和视图流照同一个函数算（`shapes.rs` 经 `same` 对照）；不认这一格的头照旧用 `args`。
