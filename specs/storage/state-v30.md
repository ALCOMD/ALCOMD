# State30 · 软件包展示观测

所有者于 2026-11-08 批准，接续 State29。迁移 `0030_package_presentations.sql` 只增加结构，不改写现有记录、事件、Plan 或恢复授权。

- `project_presentation_observations`：项目展示观测 revision、对应项目 revision、complete/partial/unobserved 状态及 SHA256 指纹。
- `project_package_presentations`：以项目/包 ID 标识锁定版本对应的本地清单展示信息。含名称、说明、关键词/别名、受控链接、撤回、legacyPackages 及清单摘要。每项目最多 4096 项、规范化 JSON 总计最多 16MiB。
- `repository_package_versions.presentation_json`：可空的规范化展示元数据。旧仓库索引为空，等待正常刷新采集；用户包从既有 manifest_json 使用同一规范化函数读取。

旧项目没有观测行，候选返回 unobserved，不将空记录视为采集完成。正常项目登记、刷新及操作发布在项目写入事务内保存观测；候选分页只读快照，不扫描磁盘或联网。项目 revision 不匹配时返回 partial，避免使用过时信息。

项目展示观测作为检查器至 Store 的瞬时载荷，不进入序列化项目快照或 Plan/Apply 冻结授权。候选缓存单独纳入展示 revision/指纹。仅收藏修改保持展示对应 revision；项目内容变化但未补采时不伪装完整。

配套公共合同为 `packages.candidates.v2`。旧 Core 不能打开 State30 数据库，不提供降级迁移；升级前应使用现有备份机制。
