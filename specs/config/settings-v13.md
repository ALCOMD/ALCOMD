# Config13 设置

Config13 是 v4 首次发布的设置基线。Core 首次初始化时创建现行设置，重开时验证文件版本、完整字段与容量。

## 软件包版本

新配置使用 `packages.showPrerelease = true` 和 `packages.preferPrerelease = false`。TOML 使用 `show_prerelease`、`prefer_prerelease`。已有配置和 v3 导入保留显示设置，缺少的偏好字段取 false。关闭显示会在同一次保存中清除偏好，再次开启只恢复可见性。

项目候选和解析目录只读取可见版本，包括仓库和本地用户包。默认安装和升级优先兼容稳定版，无合适稳定版时可选兼容预发布版。开启偏好后在兼容候选中选 SemVer 最高者。升级保留已安装版本下限。仓库管理清单和项目安装记录仍可展示预发布版。

## 设置形状

`packages.hiddenRepositoryPackages` 为设置快照的必填字段，默认 `[]`。

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

## 持久化

Config13 TOML 使用 `[packages]` 的 `hidden_repository_packages` 保存仓库和包 ID。完整文件读取及保存上限为 1 MiB，去除该列表后其余设置的编码上限为 16 KiB。

`system.hello.configSchema` 和设置结果报告 13。未知版本、未知字段或无当前权威的中断文件在写入前拒绝。有效 Config13 的写入使用同目录暂存和可恢复的替换。首次发布初始化和迁移边界见 [M11 实施计划](../../docs/exec-plans/M11-v3-migration-and-zip.md)。

实施和回归边界见[ExecPlan](../../docs/exec-plans/M7-repository-package-switches.md)。

[Schema](settings-v13.schema.json) · [契约向量](settings-v13.contract-vectors.json)

## 默认保存目录（2026-11-10 所有者调整）

`settings.paths.projectDirectory` 或 `backupDirectory` 为空时，使用用户主目录下由产品身份派生的 `ALCOMD/Projects` 和 `ALCOMD/Backups`。Windows 查询 User Profile Known Folder，macOS 和 Linux 使用 HOME。

显式路径继续优先，`directories` 仍只投影当前有效值，不回写默认路径；保留设置修订、既有资源及已接受任务的固定目的地，不自动搬迁。内部状态/配置目录不受影响，Config13 形状及版本不变。验证见[默认存储根记录](../../docs/testing/home-storage-defaults-20261110.md)。

## 默认备份压缩（2026-11-12 所有者调整）

新配置使用 `level5`（Deflate 等级 5）。显式单次选择优先于已保存的压缩选项，已接受任务和幂等重放使用冻结的选项。可用等级是 0、1、3、5、7 和 9。文件格式见 [Unity 项目 ZIP](../backups/project-zip.md)。
