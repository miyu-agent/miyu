## ST-1 列不到模型照目录挑

状态：已完成（2026-10-29）。

### 目的

试一家供应商时列不到模型、又不是 key 错的，照目录里这一家的模型挑一个推荐的发一句验证，不再直接要人凭空填模型名。项目主人 10-29 在虚拟机里以新用户身份 `miyu setup` 选 OpenCode Go：目录里它换了地址（`https://opencode.ai/inference/go/openai/v1`），新地址没有列模型的接口（404），命令行只说「Could not get the model list」，手填的模型名又写错，接不上。

### 蓝图改哪几节

- `models.md`「怎么走」第七条第 4 款：列不到、不是认证错的，模型照目录列，没写模型的照第 3 款的推荐挑一个发一句；目录里也挑不出的才 `need_model`。
- 参考 `config.md` 的 `provider.test` 同上。

### 不做什么

- 命令行那一半（要人填模型名时说清列不到的原因、给出目录里的模型让人选）归 CL-2 下。
- 不在档案里写死 OpenCode Go 的老地址：目录换地址是上游的事，跟着目录走。

### 验收

- `providers_test.rs` 的 `a_provider_that_cannot_list_is_tried_with_a_model_from_the_catalog`：列模型 404、没写模型的，照目录挑一个发一句，回 `ok`、`checked: request`、`listed: catalog`；退回修之前报红。
- 本机门禁。
