# Config11：VCC 链接偏好

状态：历史迁移输入。当前设置合同为 [Config13](settings-v13.md)。

`settings.integrations.handleVccLinks` 是默认开启的布尔值，TOML 使用 `[integrations]` 下的 `handle_vcc_links`。完整读取与指定该组的更新均须包含布尔值，未知字段、空组、null 和错误类型均拒绝。沿用 `settings.read`、`settings.manage` 和 revision CAS。

有效 Config1–10 按其原版本校验并按既有迁移规则补齐历史字段。升级 Config10 时只新增默认开启的偏好，保留其他值和 revision。旧版本文件包含新字段时拒绝。协商与设置读写报告 Config11，严格读取方需更新必需字段。RPC major、State26 和既有依赖不变。

Windows 正式构建与 Linux AppImage 按 Core 接受的偏好处理桌面注册，Windows 关闭时只移除能确认属于当前 ALCOMD 可执行文件的用户级登记。Linux 移除本应用的桌面处理器。开发和隔离运行策略见 [M7 实施合同](../../docs/exec-plans/M7-official-gui-extension-ui.md#v3-包管理行为对齐)。

[Schema](settings-v11.schema.json) · [契约向量](settings-v11.contract-vectors.json)
