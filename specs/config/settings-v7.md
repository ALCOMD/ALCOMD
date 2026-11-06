# settings.toml Config Schema 7

状态：历史迁移输入。项目所有者于2026-11-03批准本版窗口状态记忆合同；当前有效设置见[Config12](settings-v12.md)。

Config7 在 Config6 上增加 `window.rememberState`，仍由 `alcomd` 经 application 用例层持有唯一设置权威。
Config1–6 只作为经过各自严格校验的启动迁移输入；保留 revision、主题、语言、包偏好及 Hub 路径，
旧 density 依其历史合同校验后丢弃，不恢复紧凑模式。Config1–6 的窗口开关补 false；Config6 的 Unity 参数覆盖原样保留，Config5 及更早仍按原规则补参数 null。迁移在 ready 前完成，不算用户更新或推进 revision。

```toml
schema = 7
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

# 仅显式目录覆盖非空时写出 [paths] 和对应字段。
# [paths]
# project_directory = "..."
# backup_directory = "..."
```

## 窗口状态记忆

完整公开 `settings.window` 必含唯一字段 `rememberState: boolean`，默认 false。
Config7 的 TOML 必须写出 `[window] remember_state = false` 或 true；既有 Config1–6 的有效输入迁移补 false。
PATCH 省略 window 保持原值，显式 false 是有效更新；容器必须是闭合非空对象且含这个 bool。
null、字符串/数字布尔值、未知字段（包括窗口宽高、位置或最大化状态）整次拒绝。
开关沿用 settings.read/manage、串行 CAS、16 KiB 文件上限及已有原子替换，不需要 unity.manage。

Core 只保存开关。普通窗口逻辑宽高与最大化状态属于可丢弃 GUI 视图缓存，不进入 Core 配置、State17 或公开设置。
关闭默认不恢复、不记录并清除旧几何缓存；启用后下次启动恢复一次。窗口位置、最小化启动、全屏偏好不属于本合同。
缓存失败不阻止启动/关闭，原生行为验收由 GUI 实现与定向验证单独记录；配置迁移不改变真实项目或数据库。

## 默认 Unity 启动参数

完整公开 `settings.unity` 必含 `hubPath`、`defaultLaunchArguments`；后者为 `string[] | null`。
TOML 对应 `[unity].default_launch_arguments`：省略表示 null，`[]` 表示显式空数组；
数组使用单行 TOML basic strings，保留空格、引号、反斜线及转义控制字符，不做 shell 分词或展开。
公开 PATCH 省略字段表示不变，null 清除覆盖，数组整组替换；空 unity 容器、未知字段及非字符串数组整次拒绝。

null 使用产品内置默认（当前为 `[]`）；存在项目覆盖（含 `[]`）时使用项目覆盖，否则采用全局覆盖，再退到内置默认。
不拼接数组。`unity.projectLaunchConfig.get` 保留 `config.arguments` 的项目覆盖语义，并增加同级必需
`defaultArguments`（全局覆盖或内置默认）、`effectiveArguments`（该项目实际继承结果）。项目 revision 0 表示没有覆盖；
清除项目覆盖恢复继承，设为 `[]` 则显式禁止额外参数。set/clear 的 mutation 结果形状保持。

更新此字段必须同时具备 `settings.manage` 和 `unity.manage`，包括 null/空数组操作；只修改 Hub 路径仍只要求
`settings.manage`。不新增权限、method、capability 或 RPC major；读取项目投影沿用既有项目读取权限。
项目显式空覆盖由另行批准的 [State17 最小迁移](../storage/state-v17-empty-unity-override.md) 保存，Config5→6 的迁移语义保持。

复用项目参数校验：最多 64 项、每项最多 4096 UTF-8 bytes、序列化 JSON 数组最多 65536 bytes；
拒绝 NUL、Core 管理的 `-projectPath`、`-createProject`、`-openfile` 及其不区分大小写的 `=` 形式。
不建立跨 Unity 版本的 flag allowlist。配置文件总计仍不得超过 16 KiB，超出或任一字段无效时不改变已接受配置/revision。

Unity 的 flag 与值是分离的 argv 项；路径包含空格时也作为单个字符串传递，用户无需在字符串外再加 shell 引号。
Core 用 `Command.args` 字面传参，不启动 shell，不拆词、不展开环境变量。已接受启动固定其有效参数；
后续全局修改不改变已接受启动，同幂等键参数改变返回明确冲突。完整 argv 不写入 Activity、普通错误或诊断摘要。

依据：[Unity Editor command-line arguments](https://docs.unity3d.com/2022.3/Documentation/Manual/EditorCommandLineArguments.html)、
[Rust Command::args](https://doc.rust-lang.org/std/process/struct.Command.html#method.args)。

## 默认保存目录

公开 `settings.paths` 必须包含 `projectDirectory`、`backupDirectory`，各为 `string | null`。
null 表示采用系统 Documents 下由产品身份派生的 `ALCOMD/Projects`、`ALCOMD/Backups`；
有效绝对目录单独投影在结果 `directories.projectDirectory`、`directories.backupDirectory`，均为 string。
仅对没有显式覆盖的字段，默认目录由平台窄 port 只读解析，并在设置初始化/get/update 发布前解析完成；
双显式目录不依赖 Documents 可达，清除覆盖所需的默认解析失败时不提交。结果投影不把已提交更新误报失败。
默认项目根目录在 Core 初始化时尝试准备；默认备份根目录只在明确创建新备份时准备。
目录准备失败不会锁死设置界面，实际操作仍如实失败；设置读取不写项目或创建目录。

`paths` 的 PATCH 容器必须非空；字段省略表示不变，null 清除显式覆盖，string 设置覆盖。
路径最多 4096 UTF-8 bytes，非空、不含 C0/DEL 控制字符；显式覆盖由 Core 规范化并验证绝对、安全、已存在的目录。
无关设置更新不重新验证已保存目录的即时可用性；实际项目/备份操作仍使用各自的路径与文件身份安全验证。
目录不可用不能自动改用其他位置。取消选择不保存；普通目录修改沿用串行 CAS、明确失败和未知提交核对。

默认项目目录用于新建、复制、恢复项目表单的初始位置，允许当次指定其他父目录。
默认备份目录控制后续接受的新备份；已接受任务固定其目的地，已有备份按持久定位读取/恢复。
修改设置不搬迁项目或旧备份，不重定向现有任务，不更改既有 Plan/Apply、Operation、权限或 State Schema。

## 保留的设置与存储约束

appearance 保留 mode、scheme、sourceColor、motion；mode 为 system/light/dark，motion 为 system/full/reduced，
scheme 为 tonal-spot/fidelity/monochrome/neutral/vibrant/expressive/content/rainbow/fruit-salad，默认 vibrant。
sourceColor 为 canonical uppercase `#RRGGBB` 或 null；locale 为 system/en-US/zh-CN/ja-JP。
packages 保留 showPrerelease、hiddenRepositoryIds、hideLocalUserPackages；隐藏 ID 最多 256 个、canonical unique UUID、按字节排序。
unity.hubPath 及派生 unityHub 保持 [既有 Hub 合同](unity-hub-location.proposal.contract.json)。
theme Save/Cancel、普通设置即时串行保存、原设置权限及上述字段级 unity.manage、checked revision CAS、16 KiB strict UTF-8、
同目录同步临时文件/可恢复原子替换和取消一致性保持。未知字段、旧 density、非法枚举及非 nullable 容器的 null 整次拒绝。

`system.hello.configSchema`、settings.get 和成功 settings.update 返回 configSchema 7。
这是已批准的发布前公共模型扩展；严格消费者须同步新增必需 nullable 字段与项目读取投影。RPC major、method/capability 和权限名称保持；项目显式空覆盖使用上述 State17。
settings.read 授权返回配置目录与有效目录；这些路径不进入 Activity、普通错误或 Diagnostics 摘要。

机器合同：[settings-v7.schema.json](settings-v7.schema.json)；向量：[settings-v7.contract-vectors.json](settings-v7.contract-vectors.json)；
RPC：[m7-official-gui.schema.json](../rpc/m7-official-gui.schema.json)。
