## PK-6 装着的包分得出社区：本地库记是怎么来的

状态：施工完，待 core1 审（2026-11-06，sub1）。设计 `61-装包说清来源和能力.md` 第五节第 4 条；施工 PK-3 上（看一眼的 `source`、`feed`）；
蓝图 `packages.md`「本地库」「协议」的表、`protocol.md`；参考 `packages`（中英）。

### 目的

装着的包在 `package.list`、`package.info` 里的 `from` 分不出社区：从软件源装的一律写成 `official`，社区收录的包也是。本地库的
`desc` 多记一格是怎么来的，协议上照 PK-3 上定的 `source` 取值交出去，原来的 `from` 不动。

### 定了的（core1 2026-11-05、11-06）

1. `desc` 已经有一格 `source`（从哪个路径装的），新的一格叫 `kind`，取值 `official`、`community`、`file`。本地库只记家目录里装的；
   出厂的、系统加装的照层算。
2. 装的时候照 PK-3 上看一眼的 `glance::source` 记。从包文件夹、包文件装的记 `file`，升级的照这一次重记。
3. 旧库（没有 `kind` 的）核心起来时推一次、写回：
   - 有来源仓库、有 `feed`：配置里加的源，算社区；
   - 有来源仓库、没有 `feed`：照本机存的官方索引（读的时候照旧验签名）里编号、来源仓库都对得上的那一条，官方收录的算官方，不然算社区；
     索引没有、验签不过、编号不在、来源仓库对不上的不知道；
   - 没有来源仓库、记了路径的：本机文件；
   - 都没有（起来时补记的）：不知道。
   - 推不出来的不写，下次起来再推。
   - 算进起来那段的耗时：官方索引最多读一次、验一次签名，一个都不用推的不读。写回只换 `desc`，整份先写 `.desc.new` 再换上，不半写。
4. 协议：`package.list` 每一项、`package.info` 装着的那一份多 `source`（`official`、`community`、`file`、`shipped`、`distro`、`unknown`）、
   `feed`（从软件源装的才有，官方的写 `official`，同看一眼的）。`from` 不动。
5. **撞名**：`package.info` 装着的那一份原来有一格 `source`，是从哪个路径装的，只给主人。它改名叫 `path`，同 `package.info {path}`
   回应里那一格一个意思；`source` 改说是怎么来的，同 `list`、看一眼。终端、网页由 core1 合进去以后转告：原来读 `info.source`
   当路径的，改读 `path`。
6. 命令行 `pkg info` 的「来源」照 `source` 印，`unknown` 照实写「不知道」，不写成官方。连的核心没有 `source` 的照旧照 `from`。
7. 头（终端、网页）不在这一步改。

### 做什么

- **store**（`miyu-store/src/packages/local.rs`）：
  - `Desc` 多 `kind: Option<Kind>`，`Kind` 有 `Official`、`Community`、`File` 三种，没有的不写；
  - `rewrite_desc`：只换 `desc`，暂存再换上，没记过的包不写。
- **endpoint**：
  - `approve::Came` 多 `kind`，`source/fetch.rs` 的 `came` 照 `glance::source` 算；
  - `local::record` 照它记，从本机路径装的记 `file`，补的不记；
  - `local::infer` 照第 3 条推，`packages::load` 在补记以后调它，用编进来的官方公钥；
  - `source/sync.rs` 多 `stored_index`：读本机那份、验签，不上网；
  - `fill::origin` 一次读 `desc`，写 `from`、`source`、`feed`。`package.list` 的 `item.rs`、`package.info` 的 `local.rs` 都用它，原来的
    `fill::from` 并进来；
  - `package.info` 的路径那格改名 `path`。
- **命令行**（`miyu-cli/src/pkg/info.rs`、`language/pkg/info.rs`）：照 `source` 印，多「系统装的」「不知道」两种。
- **文档**：
  - 参考 `packages`（中英）：列表的表多 `source`、`feed`，`package.info` 的回应改；
  - 蓝图 `packages.md`：本地库记什么、什么时候推，列表的表多一行，「还没有的」那条去掉；
  - 蓝图 `protocol.md`：`package.info` 的格子，不替主人干活的会话不给 `path`。

### 怎么验

- store 单元：
  - 老的 `desc`（没有 `kind`）照样读得进、写出来没有这一格；
  - `rewrite_desc` 只换 `desc`、文件清单不动、不留暂存，没记过的包不写。
- endpoint 单元（`packages/local/tests.rs`）：本地库里放八种老包，签一份官方索引推一遍：
  - 官方收录的、社区收录的、配置里加的源、本机文件夹各推对；
  - 补记的、索引里来源仓库对不上的、索引里没有的推不出、不写；
  - 已经记了的不动；
  - 换一把公钥（验签不过）：靠索引的两个推不出，别的照推。
- 集成：
  - `packages_approve_source`：从软件源装了官方收录的 `app`、社区收录的依赖 `lib`，`package.list` 里 `source` 分别是 `official`、`community`，
    `feed` 都是 `official`，`from` 都照旧是 `official`；本机文件盖过去以后 `package.info` 是 `file`、没有 `feed`；
  - `packages_local`：包文件夹装的 `source` 是 `file`、路径在 `path`；核心起来补记的 `source` 是 `unknown`、没有 `path`；
  - `packages_archive`：包文件装的同上。
- 命令行单元：
  - `unknown` 印「不知道」，`official` 印「官方」，系统装的印「系统装的」；
  - 老核心没有 `source` 的照旧印「软件源」；
  - 原来那条照新协议改（`path`、`source: community`）。

### 变异

手写 12 个，都抓住：
- 软件源装的一律记官方；
- 包文件夹装的不记；
- 补记的记成本机文件；
- 配置里加的源推成官方；
- 推的时候不比来源仓库；
- 已经记了的也重推；
- 本机文件不推；
- 官方源装的不交 `feed`；
- 不知道的交成官方；
- 路径那格还叫 `source`；
- 命令行把不知道的印成「软件源」；
- 没记过的包也写 `desc`。

「没记过的包也写」第一次没抓住：测试里那个包连目录都没有，不查也会写失败。补了「目录在、没有 `desc`」的一种，抓住了。
