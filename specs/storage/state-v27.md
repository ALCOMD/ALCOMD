# State27 · 无有效发布方摘要的 HTTP 归档

`repository_package_versions` 的本地摘要约束与 resolver-ready 写入检查接受 HTTP 和 HTTPS 归档。有效的发布方摘要仍作为校验依据。缺省或格式无效时，Core 验证首次下载并固定实际 SHA-256。

迁移在一个事务内替换本地摘要列的约束和两条写入触发器，逐字节保留已有摘要和其他记录。仓库修订、索引状态、HTTP 验证器及事件水位沿用原值。迁移不修改项目文件。旧 Core 无法打开 State27。
