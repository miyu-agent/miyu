## 施工单 KR-1：`provider login` 顺手写上密钥引用

### 为什么

自定义的供应商用 `miyu provider login <名字>` 存了密钥以后，配置里还要自己写 `key = { secret = "<名字>" }`，不写请求就不带认证头（2026-11-26 core1 配沙箱时撞上：HTTP 401「no API key was sent」）。手册写着要自己写，可很容易漏。项目主人 2026-11-26 定：顺手写上引用。

### 改了什么

- `login` 存好以后 `config.get`：系统配置里自己写了这家（`providers.<名字>.` 下有项、来源是 `system` 层）、又没有 `key` 的，`config.set` 在系统层写上引用，灰字说一句。软件包带的供应商、配置里没写的不碰；问不到、写不进的不提（密钥已经存好了）。
- 字表 `key_linked`（中英）；手册 `guide/models.md`（中英）那一句；蓝图 `cli/login.md` 第 5 条。

### 怎么验

- `tests/login.rs` 新加一条：写了 `[providers.magpie]` 的配置，登录以后多一行提示、配置里有了引用；再登录不再写；没写的 deepseek 不碰。拿掉调用那一行这一条红。
- 登录测试 8 条、clippy；门禁。
