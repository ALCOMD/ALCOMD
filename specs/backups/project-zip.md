# Unity 项目 ZIP

项目目录中的所选内容直接写入 ZIP 根目录。备份保留选定压缩级别、VPM 包和 Library 选项。省略输出路径时 Core 使用设置中的备份目录和文件名生成规则。显式输出路径须为绝对 ZIP 文件路径。原子发布要求目标文件不存在。

恢复检查 ZIP 根部的 `Assets`、`Packages/manifest.json` 和 `ProjectSettings/ProjectVersion.txt`。普通 ZIP 中只接受安全的相对条目、常规文件和目录。恢复执行文件身份、并发冲突、配额和内容检查。Core 将内容解压到目标父目录中独占的同卷暂存区，按持久日志发布并登记项目。

`archivePath` 可以位于 Core 主机的任何安全位置。传入 `destinationPath` 时，Core 使用指定的绝对新项目目录。省略时，Core 使用设置中的项目目录和文件名建议，调用方可用 `projectName` 覆盖建议名。

任务回执见 [ZIP RPC Schema](../rpc/zip-backup.schema.json)。
