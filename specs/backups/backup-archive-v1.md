# ALCOMD Backup Archive v1

状态：M5 Backup Create writer/reader 已实现；M7 已批准可选 Library 缓存内容选择，归档 manifest 形状保持。

M7六档压缩合同：store/fast/level3/level5/level7/maximum分别对应ZIP Stored及Deflate 1/3/5/7/9，界面显示0/1/3/5/7/9。旧fast和maximum保持原1/9含义，新增等级进入manifest、备份记录及幂等指纹；其他归档格式与内容选择规则不变。既有归档继续可读，旧程序不承诺读取新增等级归档。

用户备份目录的新归档文件名为 `<项目目录名>-YYYY-MM-DDTHH-MM-SS.sss.zip`，时间为创建时间的本地表示；保留中文，文件系统非法字符替换、过长项目名截短。最终名称冻结在内部归档证据中，重启恢复不重新计算，已有文件不覆盖。已有 `<BackupId>.zip` 归档及缺少新命名证据的旧任务继续按原定位读取；内部 `backup-v1` 对象定位保持UUID命名，不搬迁旧文件。归档身份仍由既有定位、文件身份和摘要确定。

ZIP 根恰好由 `backup.json` 和 `project/` 构成。ZIP64 可用，entry
只允许 Stored 或 Deflate；写入必须按规范化相对路径稳定排序并流式执行。不得使用一次性 convenience
extract/write 绕过 path、link、collision、quota 或 streaming 检查。

`backup.json` 必须匹配 `backup-v1.schema.json`，不得包含绝对路径、Principal、数据库/cache locator、
credential/token、journal 或内部 partial/final locator。`excludedPackages` 只在
`excludeVpmPackages=true` 时包含实际排除的 locked package；`packagesRequireResolve` 固定为 `true`。

## 配额

| 限制 | v1 值 |
| --- | ---: |
| archive bytes | 68,719,476,736（64 GiB） |
| entries | 500,000 |
| single regular file | 34,359,738,368（32 GiB） |
| total uncompressed | 137,438,953,472（128 GiB） |
| path depth | 128 |
| normalized relative path | 1,024 UTF-8 bytes |
| expansion ratio | 10,000:1 |

所有计数使用 checked `u64` 算术；边界测试使用注入的小配额，不创建巨型 CI 文件。

## 精确 include/exclude profile

机器可读权威表为 `backup-profile-v1.json`。规则按规范化路径组件判断，并使用平台安全的大小写处理：

- 排除根目录 `Logs/`、`Obj/`、`Temp/`；
- 排除任意深度名为 `.git/` 的目录；
- 排除每个根目录名以 ASCII 大小写不敏感 `Library` 开头的目录；
- 明确 `includeLibrary=true` 时，仅标准根目录 `Library/`（ASCII 大小写不敏感）改为普通项目内容并完整保留缓存；`Library-foo/` 等其他前缀目录仍按原策略排除；
- 唯一例外是该根目录的直接文件 `LastSceneManagerSetup.txt`，不递归恢复其子目录同名文件；
- 其他内容默认包含，包括 `MemoryCaptures/`、`UserSettings/`、`.vscode/`、`.idea/`；
- `ProjectSettings/ProjectVersion.txt`、`Packages/vpm-manifest.json`、`Packages/manifest.json` 必须保留。

`excludeVpmPackages=false` 时 `Packages/` 作为普通项目内容处理。为 `true` 时，只排除已经成功解析的
normalized locked VPM set 中、package ID 有效、root containment/目录类型/locked identity 均验证通过的
`Packages/<package-id>/`。不得根据目录名、前缀、package.json 或当前 catalog 猜测。manifest、unlocked/
embedded package 与 unknown Packages child 必须保留；任何拟排除项不一致都 fail closed。

Library 选择只改变 inventory 内容，不增加 `backup.json` 或 BackupRecord 字段、不改变恢复格式。
未显式传参时由 Core 在接受任务时从同一权威设置快照解析压缩、包及 Library 偏好与默认目录；已接受请求固定有效值。
保留的旧请求省略 Library 时固定为 false，原 false 的幂等指纹字节不变；新 true 进入指纹。
使用同幂等键重放时，省略的选项沿用原接受结果，显式不同选项仍拒绝，不重读已变化的默认设置或重定向任务。

## 一致性与恢复

inventory fingerprint 由稳定排序的 normalized relative path、类型、文件身份、大小和必要 bounded
metadata 构成，不含绝对路径。每个文件 no-follow 打开，打开后验证 identity，流式复制，结束后复验
identity/size/mtime。观察到变化返回 `project_changed_during_backup` 并隔离或丢弃 partial。

内部可采用 `partial/<OperationId>.zip.part` 与 `objects/<BackupId>.zip`，但该布局不是公共合同。最终对象
必须 create-new/atomic publish，不得覆盖。真实 kill/restart 验收点固定为 `inventory_ready`、
`archiving`、`archive_ready`、`publish_intent`、`archive_published`、`state_committed/response`。
