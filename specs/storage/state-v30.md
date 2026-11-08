# State30 · 仓库展示元数据

repository_package_versions.presentation_json 保存仓库索引中的规范化别名、关键词、撤回和 legacyPackages 信息。既有描述、依赖与链接列复用；用户包从既有 manifest_json 采用同一规范化入口读取。

此前开发实现增加的两张项目展示观测表已按所有者要求撤除，新建 State30 不再创建它们；已有开发数据由 [State31](state-v31.md) 清理。项目本地展示信息在正常请求时由 Core 读取，不依赖数据库展示副本或补采动作。
