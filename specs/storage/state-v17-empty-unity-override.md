# State17：显式空 Unity 参数覆盖

状态：项目所有者已批准并注册为生产迁移。

本轮全局默认参数及项目显式空覆盖的定向测试确认 State16 的
`project_unity_launch_config.arguments_json` 约束要求数组长度 1–64；没有记录表示继承，
因而现有结构无法存储“该项目不使用任何附加参数”。不使用特殊参数或其他表旁路表达。

最小迁移：State16→17，在一个事务中重建这一张表，只将数组长度约束改为 0–64。
保留同名列、主键、外键、STRICT、JSON 类型、64 项/65536 字符上限、revision 和时间约束；
逐行原样复制全部既有记录。其他表、项目文件、全局设置、事件和幂等记录不变。
已有记录继续表示自定义参数，没有记录继续继承；不凭空创建项目覆盖。

生产迁移：[`0017_empty_unity_launch_override.sql`](../../crates/alcomd-store/migrations/0017_empty_unity_launch_override.sql)。

核心 DDL：

```sql
CREATE TABLE project_unity_launch_config_v17 (
    project_id TEXT PRIMARY KEY REFERENCES projects(project_id) ON DELETE CASCADE,
    arguments_json TEXT NOT NULL CHECK (
        json_valid(arguments_json) AND json_type(arguments_json) = 'array'
        AND json_array_length(arguments_json) BETWEEN 0 AND 64
        AND length(arguments_json) <= 65536
    ),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;
INSERT INTO project_unity_launch_config_v17 SELECT * FROM project_unity_launch_config;
DROP TABLE project_unity_launch_config;
ALTER TABLE project_unity_launch_config_v17 RENAME TO project_unity_launch_config;
```

迁移在 Core ready 前由既有 Store 初始化链执行，成功后 `CURRENT_DATA_SCHEMA` 与 hello `dataSchema` 为17。
失败保留原有 State16 权威并拒绝启动；事务回滚不留下中间表。

DB 只放宽空数组，不增加参数类型或 flag 规则；既有 application 校验继续要求字符串数组、64项、
每项4096 UTF-8 bytes、JSON总计65536 bytes、无 NUL，并拒绝 Core 项目选择器。
没有记录继续继承，写入 `[]` 是显式空覆盖，清除记录恢复继承。

定向测试覆盖真实 State16 升级、所有既有表行与事件序列保持、无记录不补行、显式空数组重开、
STRICT/外键级联、非数组/超限/非法 revision 和时间拒绝，以及复制失败回滚。
旧历史迁移文件保持原样；没有新 RPC method、permission、依赖或项目文件变更。
机器合同见 [state-v17-empty-unity-override.contract.json](state-v17-empty-unity-override.contract.json)。
