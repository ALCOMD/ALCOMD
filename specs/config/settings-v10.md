# Config10：持久全屏模式

状态：2026-11-03项目所有者批准；当前有效设置合同。State18保持。

Config10在Config9基础上增加`window.fullscreen: boolean = false`，TOML为`[window] fullscreen = false`。它是Core权威偏好，跨GUI启动保留；GUI仅通过既有settings.get/update、settings.read/manage与串行CAS保存。全屏切换由GUI主窗口薄适配器应用，不增加Core窗口业务或数据库结构。

完整window对象必须同时包含rememberState与fullscreen两个非nullable bool；PATCH window为闭合非空对象，可独立省略任一字段表示保持。显式null、空对象、非法类型或未知字段整次拒绝，不部分应用同行合法字段。窗口卡片恢复默认经同一次既有更新将两个偏好恢复false，GUI退出全屏；不新增重置RPC、权限或依赖。

## 迁移

有效Config1–9先严格校验其原版本，再迁移为10。保留revision及全部已有设置，fullscreen仅补false；旧版本输入不能夹带fullscreen，即使值为false也按未知字段拒绝。Config7–9保存的rememberState继续保留，Config1–6仅按既定规则补false。其他历史默认、density删除、目录、Unity参数和六档备份压缩语义保持。

成功system.hello、settings.get/update均报告configSchema10。公共读取模型增加必需字段是已批准的发布前合同扩展，严格消费者须同步模型；RPC major仍为1，State18与已有任务/归档不变。Config9六档压缩和其他字段语义见[历史Config9](settings-v9.md)。

机器合同：[settings-v10.schema.json](settings-v10.schema.json)；迁移及接受/拒绝向量：[settings-v10.contract-vectors.json](settings-v10.contract-vectors.json)。
