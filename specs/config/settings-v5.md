# settings.toml Config Schema 5

状态：保留的 Config5 启动迁移输入；当前设置合同见 [Config7](settings-v7.md)。以下保留 Config5 的原始字段与验证形状。

Config5 在 Config4 上增加 `paths`，仍由 `alcomd` 经 application 用例层持有唯一设置权威。
Config1–4 只作为经过各自严格校验的启动迁移输入；保留 revision、主题、语言、包偏好及 Hub 路径，
旧 density 依其历史合同校验后丢弃，不恢复紧凑模式。迁移在 ready 前完成，不算用户更新或推进 revision。

```toml
schema = 5
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

# 仅显式目录覆盖非空时写出 [paths] 和对应字段。
# [paths]
# project_directory = "..."
# backup_directory = "..."
```

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
theme Save/Cancel、普通设置即时串行保存、原 settings.read/manage、checked revision CAS、16 KiB strict UTF-8、
同目录同步临时文件/可恢复原子替换和取消一致性保持。未知字段、旧 density、非法枚举及非 nullable 容器的 null 整次拒绝。

`system.hello.configSchema`、settings.get 和成功 settings.update 返回 configSchema 5。
这是已批准的发布前公共模型扩展；严格消费者须同步必需 paths/directories 字段。RPC major、method/capability、权限、State Schema 不变。
settings.read 授权返回配置目录与有效目录；这些路径不进入 Activity、普通错误或 Diagnostics 摘要。

机器合同：[settings-v5.schema.json](settings-v5.schema.json)；向量：[settings-v5.contract-vectors.json](settings-v5.contract-vectors.json)；
RPC：[m7-official-gui.schema.json](../rpc/m7-official-gui.schema.json)。
