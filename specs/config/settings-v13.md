# Config13 存储库内软件包开关

状态：2026-11-09，所有者已批准公开设置字段、Config13 升级和容量边界，已实施并通过定向验证，范围见 ExecPlan。Config1–12 仅保留为迁移输入；运行实例已于2026-11-10按所有者要求升级，验证见[记录](../../docs/testing/runtime-update-20261110.md)。

## 设置形状

既有 settings DTO 的 `packages` 新增必填 `hiddenRepositoryPackages`，默认 `[]`：

```json
{
    "hiddenRepositoryPackages": [
        {
            "repositoryId": "00000000-0000-4000-8000-000000000102",
            "packageIds": ["com.example.unused"]
        }
    ]
}
```

`settings.update` 中该字段可省略，省略保留原值；提供时以完整清单替换，`[]` 清空，`null` 非法。沿用 revision CAS、`settings.read`、`settings.manage` 及现有错误合同。单仓库保存需基于最新已确认完整设置保留其他分组；冲突不自动覆盖或重放。

分组最多256项，所有分组合计最多4096个包ID。组内非空，包ID长度1–128字节，首字符为ASCII字母/数字，后续允许ASCII字母/数字及`.`、`_`、`-`，与现有包身份规则一致，大小写保持。仓库ID必须为canonical lowercase UUID；分组的仓库ID不得重复，组内包ID不得重复。写请求无序时规范化排序，重复或非法值拒绝；持久数据须已规范化，不能静默修复损坏配置。空分组由客户端在移除最后一个包偏好时删除。

## 行为

清单中的包仅对对应仓库关闭，全部版本及后续同步得到的新版本一起关闭；同名包的其他来源独立。整库关闭或包关闭任一命中即排除该来源。关闭不卸载、不删除缓存/包文件、不删除仓库，也不改项目清单。

Core的普通候选/提供方/求解/计划与提交前复验使用同一配置权威。旧计划和手工来源不能绕过。已安装包仍由项目提供事实；没有其他开启来源即来源未知，本地卸载资格保持。raw仓库浏览与包详情是筛选管理入口，可以显示和重新开启关闭项；该管理例外不扩散到项目候选。

## 持久化与兼容边界

Config13 TOML的 `[packages]` 增加 `hidden_repository_packages` 分组，保存相同的仓库/包ID偏好，不存名称、版本或元数据。完整文件有界读取及保存上限为1MiB；排除新字段后的旧字段仍保留16KiB基线体积预算与原有字段限制。Config1–12文件仍按原16KiB上限检查，不能借升级放宽旧格式校验。

已有合法配置升级时仅增加空清单并写入新Schema，原设置及revision保持；旧Schema中夹带新字段拒绝。`system.hello.configSchema`及设置成功结果报告13；严格客户端须同步新增字段。不增加SQL Schema、RPC major、能力名、权限或迁移删除项；不双写、不维护Config12运行实现。旧Core不支持读取Config13，回退运行版本须使用升级前备份，不静默删去新偏好。

实施和回归边界见[ExecPlan](../../docs/exec-plans/M7-repository-package-switches.md)。

[Schema](settings-v13.schema.json) · [契约向量](settings-v13.contract-vectors.json)

## 默认保存目录（2026-11-10 所有者调整）

`settings.paths.projectDirectory` / `backupDirectory` 为 null 时，使用用户主目录下由产品身份派生的 `ALCOMD/Projects`、`ALCOMD/Backups`。Windows 查询 User Profile Known Folder；macOS/Linux 使用 HOME，不再使用 Documents 或 XDG 文档目录。此规则取代历史 Config5–7 说明中的 Documents 基准；旧版本仅为迁移输入。

显式路径继续优先，`directories` 仍只投影当前有效值，不回写默认路径；保留设置修订、既有资源及已接受任务的固定目的地，不自动搬迁。内部状态/配置目录不受影响，Config13 形状及版本不变。验证见[默认存储根记录](../../docs/testing/home-storage-defaults-20261110.md)。

## 默认备份压缩（2026-11-12 所有者调整）

新配置及没有备份选项的旧配置采用 `level5`（真实Deflate等级5）；替代此前的 `fast` 默认。已经保存的压缩选项保持，已接受任务和幂等重放不重新解析默认值，显式单次选择仍优先。0/1/3/5/7/9档位及Config13形状、版本保持。新备份文件名与验证范围见[备份修正计划](../../docs/exec-plans/M7-backup-names-default-compression.md)。
