## 施工单 ED-1：edit 没读过的文件也能改

状态：已合（2026-10-24）。经过：项目主人 2026-10-24 要去掉 `edit`「改之前必须先读」的限制；读过、之后文件又被改了的照旧拦（推荐项），`write` 不动。设计 `10-自带软件.md` 第六节「`edit` 的细则」；图纸 `tools/edit.md`「是什么」「怎么走」第 5 条、出错表、「守着它的」。

### 目的

`edit` 要 `old_string` 和原文对得上、唯一才改，没读过也改不到别处去；先读一遍只是为了过这道检查，多花一次调用和读进来的那些 token。去掉以后她知道原文时能直接改。

### 做法

1. `common.rs` 多一个 `Common::stale`：只拦看过、现在的内容和她看到的不一样的；没看过的放过去。`edit.rs` 改用它，`write` 照旧用 `unseen`。
2. `tools/edit.json` 的说明去掉「The file must have been read first.」，登记簿换指纹、`prompts.md` 重新生成。`common/not-read.txt` 只剩 `write` 用，字不改。

### 不做

- `write` 照旧要先读：整份覆盖，没读过就写会把不知道的内容整个盖掉。
- 「读过以后又被改了」照旧拦：照着旧的内容改，会盖掉人或者别的程序刚做的改动。

### 验收

- `crates/miyu-basesystem/tests/edit.rs` 的 `edits_what_she_has_not_read`：没读过的对得上就改，对不上照旧报没找到、文件不动；改代码之前它是红的。
- `what_is_not_edited`：读过以后又改了的照旧不改。
- 本机门禁全过。
