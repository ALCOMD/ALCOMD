# settings.toml Config Schema 9

状态：Config9历史迁移输入；当前合同为[Config10](settings-v10.md)。所有者选择NanaZip截图的六档压缩等级，沿用已说明的最小Schema迁移及已有数据保留方案。

Config9与Config8的字段、权限、默认值、串行CAS和16 KiB边界相同，仅扩展`backups.compressionMode`的合法枚举。
`system.hello.configSchema`和成功settings.get/update返回9。GUI默认设置及单次备份共用以下档位：

| 等级 | 显示名称 | RPC / TOML / CLI值 | 实际编码 |
| --- | --- | --- | --- |
| 0 | 仅存储 | store | ZIP Stored |
| 1 | 极速压缩 | fast | Deflate 1 |
| 3 | 快速压缩 | level3 | Deflate 3 |
| 5 | 标准压缩 | level5 | Deflate 5 |
| 7 | 最大压缩 | level7 | Deflate 7 |
| 9 | 极限压缩 | maximum | Deflate 9 |

默认仍为fast（1级），原maximum仍为9级；不把旧值重解释为7级。仅上述六个字符串有效，不接受数值、null、其他等级或同义别名。
完整设置必含该字段；PATCH省略表示保持，空容器仍拒绝。单次备份允许覆盖，不回写默认设置；接受任务时固定等级，后续修改设置不改变任务或幂等重放。

## 历史输入

有效Config1–8按原版本严格校验后迁移，保留revision及全部已有值，只有Config1–7补入Config8规定的新默认。Config8不接受level3/level5/level7，避免用旧版本号标记新合同。
原density仅在历史输入按既定规则校验后移除；路径、主题、语言、项目自动发现、软件包/Library选项、Unity参数和窗口偏好均保留。
Config9只改枚举，不增加配置字段、依赖或权限。

State18用于扩展备份记录的等级约束，见[state-v18-backup-compression.md](../storage/state-v18-backup-compression.md)。Archive v1同步扩展压缩枚举，旧归档及任务含义保留，旧程序不承诺读取使用新增等级的归档。

机器合同：[settings-v9.schema.json](settings-v9.schema.json)；向量：[settings-v9.contract-vectors.json](settings-v9.contract-vectors.json)。Config8其他语义见[历史合同](settings-v8.md)。

## 当前GUI保存方式

主题与其他偏好共用串行自动保存队列，mode/sourceColor/scheme只提交实际修改字段，不保留独立Save/Cancel或主题草稿离开确认。HEX输入仅在完整合法时写入；失败保留意图并经通用Retry先重读核对，未知已提交不重复写，其他字段保持。恢复默认仍由现有settings.update完成，并在GUI确认后只恢复该卡片字段；本次仅改变GUI交互，不改变Config9/RPC形状或权限。旧Config3/4文档中的主题显式保存描述为历史交互。
