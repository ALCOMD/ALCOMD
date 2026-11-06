# Config10：持久全屏模式

状态：历史迁移输入。当前设置合同为 [Config12](settings-v12.md)。

Config10在Config9基础上增加`window.fullscreen: boolean = false`，TOML为`[window] fullscreen = false`。它是Core权威偏好，跨GUI启动保留；GUI仅通过既有settings.get/update、settings.read/manage与串行CAS保存。全屏切换由GUI主窗口薄适配器应用，不增加Core窗口业务或数据库结构。

完整window对象必须同时包含rememberState与fullscreen两个非nullable bool；PATCH window为闭合非空对象，可独立省略任一字段表示保持。显式null、空对象、非法类型或未知字段整次拒绝，不部分应用同行合法字段。窗口卡片恢复默认经同一次既有更新将两个偏好恢复false，GUI退出全屏；不新增重置RPC、权限或依赖。

## 迁移

有效Config1–9先严格校验其原版本，再迁移为10。保留revision及全部已有设置，fullscreen仅补false；旧版本输入不能夹带fullscreen，即使值为false也按未知字段拒绝。Config7–9保存的rememberState继续保留，Config1–6仅按既定规则补false。其他历史默认、density删除、目录、Unity参数和六档备份压缩语义保持。

成功system.hello、settings.get/update均报告configSchema10。公共读取模型增加必需字段是已批准的发布前合同扩展，严格消费者须同步模型；RPC major仍为1，State18与已有任务/归档不变。Config9六档压缩和其他字段语义见[历史Config9](settings-v9.md)。

机器合同：[settings-v10.schema.json](settings-v10.schema.json)；迁移及接受/拒绝向量：[settings-v10.contract-vectors.json](settings-v10.contract-vectors.json)。

## TOML读取与恢复

读取复用workspace锁定的`toml 1.1.4`，直接解码为闭合的类型化结构，避免经过仅容纳`i64`的通用值丢失`u64` revision。合法行尾注释、引号、转义及多行数组按TOML语法读取，再执行各Config版本自身的字段、类型及取值验证；不要求用户文件与规范化写出逐字相同。未知字段、重复定义、非法类型、无效UTF-8及NUL仍拒绝。16 KiB限制、正数revision/CAS、历史版本边界和规范化写出保持。

替换流程依次完成独占同目录临时文件写入及文件同步、目录同步、旧目标改名为备份及目录同步、临时文件改名为目标及目录同步；核对已发布内容后清理备份并再次同步目录。失败可发生在实际发布之后，调用方必须失效旧缓存并重新核对；不以失败为由删除用户配置或恢复默认值。恢复仅选择通过原版本校验的目标或备份，先同步确定的目录状态，再清理替换残留。无效目标及其备份、临时文件保留供核查，不擅自覆盖。

目录同步复用现有platform原语；macOS目录`fsync`的明确`EINVAL`沿用该原语记录的不支持语义，不宣称获得该平台无法提供的保证。故障注入与实际终止测试写入子进程证明恢复路径，不能替代真实掉电及各平台文件系统实证。
