# settings.toml Config Schema 8

状态：Config8历史迁移输入；当前合同为[Config10](settings-v10.md)。项目所有者于2026-11-03批准项目自动发现及备份默认选项，该版本保留原三档压缩枚举。

Config8 保留 Config7 的全部字段及语义，新增 `projects`、`backups` 两个闭合设置对象。
本版合同实施时 `system.hello.configSchema`、settings.get和成功settings.update均返回8；当前版本见Config10。
State17、RPC v1、方法名、capability、权限名称保持；严格消费者必须同步新增必需设置对象。

```toml
schema = 8
revision = 1
locale = "system"

[appearance]
mode = "system"
scheme = "vibrant"
motion = "system"

[packages]
show_prerelease = false
hidden_repository_ids = []
hide_local_user_packages = false

[window]
remember_state = false

[projects]
auto_discover = true

[backups]
compression_mode = "fast"
include_vpm_packages = true
include_library = false
```

`paths.projectDirectory` / `paths.backupDirectory` 继续保存 nullable 显式目录覆盖，有效目录继续由
`directories` 投影。Config7 的窗口开关、Unity 参数及 Hub 路径、外观、语言和包偏好均保持；
详见 [Config7 历史合同](settings-v7.md)。16 KiB strict UTF-8、串行 checked CAS、原子替换和取消一致性不变。

## 项目自动发现

完整 `settings.projects` 必含 `autoDiscover: boolean`，默认 true；TOML 为 `[projects].auto_discover`。
PATCH 省略对象表示保持，存在时必须含该 bool；空容器、null、未知字段和非法类型整次拒绝。
修改该字段需要 `settings.manage` 与现有 `projects.manage`；自动发现开启时更改项目默认目录也需要两者。
关闭自动发现时，只改项目默认目录仍只需 `settings.manage`。不新增公开权限或方法。

Core 在启动、设置变化及每 30 秒合并调度有界扫描，不阻塞 ready。只枚举有效项目根目录的直接子目录，
单轮最多 4096 项、不递归、不跟随 symlink/reparse，使用现有 ExactRoot 读取和 Store 文件身份查重。
重复登记不新增记录或事件；不可达根目录、无效项目或扫描上限不删除已有记录，也不写入工程。
每项接受与设置更新串行，关闭或换目录后不再接受旧轮后续登记。只从列表移除而未移走的项目会再次发现。
`projects.list` 继续纯读取，不增加永久排除状态或数据库迁移。

## 备份默认值

完整 `settings.backups` 必含三项：

- `compressionMode: "store" | "fast" | "maximum"`，默认 fast，复用既有三档压缩。
- `includeVpmPackages: boolean`，默认 true。
- `includeLibrary: boolean`，默认 false。

TOML 对应 `[backups]` 中 `compression_mode`、`include_vpm_packages`、`include_library`，均必需。
PATCH 三字段可以独立省略；空对象、null、未知字段和非法类型整次拒绝。更新只要求 `settings.manage`。

新备份表单从权威设置初始化并允许单次覆盖。所有入口省略对应请求字段时都在 application 用例中解析当前默认值；
显式值优先，软件包正向选项映射既有 `excludeVpmPackages` 的反值。接受时从同一配置快照固定有效目的地及选项，
后续设置修改不能改变已接受 Operation 或同幂等键重放。Library 内容和指纹合同见现有 Backup Archive v1 规范及 M7 ExecPlan。

## 历史输入迁移

Config1–7 只作为经过各自严格校验的启动迁移输入，新增两个设置对象补上述默认值。
保留原 revision、全部既有字段及已保存的 false/空数组/null；仅历史 density 依原规则校验后丢弃。
Config1–7 不接受新对象，即使值恰好为默认值也拒绝；Config8 不能缺失任一新增必需字段。
迁移在 ready 前原子完成，不算用户设置更新、不推进 revision、不修改 State17 或真实项目。

机器合同：[settings-v8.schema.json](settings-v8.schema.json)；
向量：[settings-v8.contract-vectors.json](settings-v8.contract-vectors.json)；
RPC：[m7-official-gui.schema.json](../rpc/m7-official-gui.schema.json)。
