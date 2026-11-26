# BO-1　卸包时删掉它装着的那一版的包文件

## 为什么

从软件源装包时，下载的包文件放在 `var/cache/packages/downloads/`，装完以后照 pacman 的做法留着，只有 `miyu pkg clean` 才删，而且它只删不是装着那一版的。卸掉一个包以后，它的包文件还在，要再敲一次 `miyu pkg clean` 才清得掉。

2026-11-27 的基准测试量到了这一点：照「可选的部分一律关掉」卸掉语义模型（`embed`）以后，数据目录是 66 MiB，其中 59 MB 是 `embed` 的包文件。同一批里 LiteMiyu 的数据目录是 6.6 MiB。

卸掉的包留着几十兆的包文件没有用处。2026-11-27 项目主人定：卸一个包的时候，把它的下载文件一起删掉，其余照旧。

## 怎么改

`package.remove` 卸成以后，同删状态目录的那一步一起，把下载缓存里这个包装着的那一版的包文件删掉：`<编号>-<版本>.miyu` 和带系统的 `<编号>-<版本>-<系统>.miyu`。认文件名的规则和 `package.clean` 认「装着的那一版」是同一条。

只删这一版，理由有两条：

- 文件名是 `<编号>-<版本>` 拼的，编号里可以有连字符。只拿编号当前缀去认，`app` 会认到 `app-extra-1.0.miyu`。带上版本就没有歧义。
- 别的版本的包文件本来就归 `package.clean` 删，这一步不改它的职责。

不删的照旧：还装着的包的包文件、别的包的、下了一半的。出厂的、系统加装的包卸掉只是记一笔，没有下过，什么都不删；从本机文件夹、本机包文件装的也没有。

行为上有一处变化：卸了以后再装同一版，要重新下载。以前这种情况直接用缓存里的包文件。降级（装缓存里的旧版本）、升级都不受影响。

协议的形状不变：`package.remove` 的回应不加格子，删了的记一行 `INFO package download removed`，带释放的字节数。

## 改动

- `crates/miyu-endpoint/src/packages/source/clean.rs`：认文件名的那一段拆成 `is_of`，`sweep` 和新加的 `forget` 共用；`forget(cache, id, version)` 删这一版的包文件，交回字节数。
- `crates/miyu-endpoint/src/packages/remove.rs`：卸之前记下装上的那一份的版本，卸成以后调 `forget`。
- `crates/miyu-endpoint/src/packages/source/clean/tests.rs`：只删这一版，别的包、别的版本、编号是它的前缀的包、下了一半的都留着。
- `crates/miyu-endpoint/tests/packages_fetch.rs`：从软件源装了再卸，它的包文件没了、还装着的依赖的留着；「缓存里有就不再下」那一段改成先把包文件放回缓存再装，这一路照旧有测试。
- 蓝图 `packages.md`「装卸」第 7 条、`protocol.md` 的 `package.remove` 和 `package.clean`、`cli/pkg.md` 的 `remove`；设计 `37-软件源.md` 第四节第 5 条。
- 用户文档（中文、英文）：`guide/data.md` 的下载缓存那一行、`guide/packages.md` 卸载时删什么和 `clean` 的说明、`reference/packages.md` 的 `package.clean`。

## 验收

- 门禁全绿。
- 沙箱里用 rc.4 之后的这一版照基准测试的初始化走一遍：装上 `embed` 再卸掉，`var/cache/packages/downloads/` 里没有它的包文件。
