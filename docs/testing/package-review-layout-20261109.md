# 软件包确认弹窗排版与更新错误排查

日期：2026-11-09。所有者要求确认弹窗按v3组织显示，随后报告版本更新按钮无确认弹窗，并质疑真实项目文件的读取边界。当前范围见[M7合同](../exec-plans/M7-package-v3-parity-audit.md)。

## 已完成的呈现修正

独立实现v3的内容组织，不复制/移植参考源码：同类变更分组并按标识稳定排序，每项用整行描述及名称强调；移除项不再拆成动作/名称/旧版本三行。依赖冲突按被移除包汇总依赖方，在主视图移除重复的版本范围和圆点。保留所有依赖方、名称回退、Core冻结数据和RPC原标识，不从GUI推断Core未给出的移除原因。旧资产路径与所属名称保持。

使用既有Material Dialog操作槽固定Cancel/Apply，正文由组件负责滚动；无变更、错误后关闭、忙碌时禁用、取消及焦点恢复保持。统一顶部风险说明和冲突小标题，未增加审批或等待。

前端检查与生产构建通过，42项现有浏览器回归通过，包含名称/未知依赖方回退、冲突每包一组、无圆点、固定操作槽、nullable版本、取消/Apply及原请求身份。已查看中文常规与520×650窄窗口截图：`target/m7-package-review-layout-update-20261109/output/playwright/review-zh.png`和`review-zh-narrow.png`。证据目录中的`frontend-check-final.log`、`frontend-build.log`、`browser.log`及`inputs.json`绑定本轮输入。

手工浏览器仅使用合成客户端，不连接真实Core。首次打开时回归服务器刚退出，得到连接拒绝；改为预览同一已构建夹具后通过。开发模式原有Material/React ARIA及Lit提示保留。验证会话与独立预览均已关闭。

## 更新错误与访问边界

通过`logs.read`读取的升级记录多次返回`PackageLegacyCleanupRequired`，并非持续等待；也有成功计划记录。此前诊断直接扫描真实项目Assets目录，读取443份meta，并检查了5份缓存包声明，发现一个80532字节的FBX meta触发当前Core的65536字节整文件限制。该meta检查会在包声明了旧资源GUID而需定位移动资产时执行；不读取FBX模型内容。

所有者随后明确澄清：AI可读取其授权测试用项目，质疑的是ALCOMD普通包操作是否越界访问。此前将其解释为AI诊断读取未获准并不准确；没有修改项目文件。流式读取未实施，不能将大小限制修复当作访问范围已经合理。

仅用合成模型meta构造的候选升级Plan/Apply测试复现同一错误（`upgrade-before.log`），没有通过，不计完成。该候选移出正式测试文件保存在证据目录`large-meta-probe.rs`。后续已按所有者要求独立修正更新/重装跳过旧声明，包含80532字节无关meta的隔离真实事务回归通过。所有者最终选择保留官方递归GUID发现，取消此前范围收窄候选；流式读取修正与大meta/深层移动资源真实事务验证详见[读取范围合同](../exec-plans/M7-legacy-discovery-scopes.md)。失败呈现按此前明确要求保持。

只读参考证据：v3的`unity_project/find_legacy_assets.rs`先按声明路径，再对未解决GUID搜索Assets元数据；现有RPC合同的legacy章节也描述了项目相对路径/GUID匹配。这与本次诊断是否获准直接访问真实项目是两个边界。[Unity 2022.3元数据文档](https://docs.unity.cn/cn/2022.3/Manual/AssetMetadata.html)说明meta包含资产ID及导入设置，不能由文件较大推断其异常。

未更新运行Core/GUI、未进行真实包Apply、未提交/推送。原生端到端、完整workspace及三平台Hosted未执行；本轮通过仅覆盖上述GUI呈现范围，M7/H7保持。
