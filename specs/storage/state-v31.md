# State31 · 移除项目展示数据库副本

所有者于 2026-11-08 明确要求撤掉新增的项目展示副本及补采流程。0031_remove_project_presentations.sql 仅删除已有开发数据库中的 project_package_presentations 与 project_presentation_observations；允许二者已不存在的新建数据库走同一迁移。

项目、仓库、索引、计划、操作和事件均保留，不改写冻结 JSON 或 Apply/recovery authority。State30 新建脚本仅增加仓库 presentation_json，不再创建项目展示表。普通候选查询通过 Core 直接读取项目文件，结果仅暂存在有界内存快照，不写数据库；没有“未采集后补写”的持久状态。

这是开发数据格式变更，不代表产品曾发布版本。仓库索引中的规范化展示字段继续保留，不属于项目文件的额外数据库副本。
