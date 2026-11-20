## BS-1 下上：软件包列表带组

状态：主会话做。设计 44（基础系统拆包）第三节第 2 条：`package.list` 每一项多一格 `groups`，回应顶上多一格 `groups`（编号、名字、说明照连接的语言挑），头照它把组里的包收在一起。BS-1 下拆成三步：下上（这一步）、下中（装卸整个组）、下下（斜杠命令的拥有者）。

### 一、做了什么

1. **组表的读法**：`miyu_config::package::groups::read`，读出厂的 `resources/packages/groups.toml`，一个组一段：编号写法同包的编号，`name` 必写、`summary` 可写，别的格、写错的说哪一段哪一格。
2. **列表**：每一项多 `groups`（清单写的组，不在组里的不写）；回应顶上多 `groups`，组表里每个组 `{"id", "name", "summary"?}`，照连接的语言挑。组表读不成的记一行、当没有。成员看到的几格里也有 `groups`，顺带补上 U23-R 漏掉的 `author`、`upstream`（清单里写的，不是这台机器的）。
3. **资源的一致**：出厂清单写的组都要在组表里，没有哪个包的编号和组名撞（测试守）。
4. **文档**：协议参考 `packages.md` 中英两份写上 `groups`，顺带删掉 BS-1 上中漏改的 `required`、`package_required`（`errors.md` 两份、蓝图 `protocol.md`、`package-pages.md` 同）。

### 二、怎么验

1. `miyu-config` 的 `package/groups/tests.rs`：照先后读、语言表、四种写错的说哪里。
2. 端点 `package_groups.rs`：出厂清单的组都在组表里、不和包撞名；列表顶上的组照连接的语言，`shell` 写 `basesystem`、不在组里的不写，成员也看得到。
3. 手写变异 6 个全抓到：项里不写组、顶上不写组、成员看不到组、组不带说明、组表许没名字、组表许坏编号。
4. 本机门禁。
