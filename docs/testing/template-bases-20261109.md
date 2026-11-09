# 空白与基础模板恢复验证

2026-11-09；main基线`b55f91187d9f94f511bc0b8f5f021fa235b176e2`加工作区。用户明确要求与提前批准见本次对话，合同见[ExecPlan](../exec-plans/M7-template-bases.md)。保留已有50ms悬停与按下预取修改，不提交/推送或替换运行实例。

## 已实现

空白为锁定内置目录记录，默认供新建项目与手动建模板选择；实际创建Assets、ProjectSettings、空UPM/VPM清单，并写完整Unity版本、项目名及独立GUID。常规模板固定Editor，空白按用户选择版本生成。普通缺版本导入仍拒绝；只有保留ID的Blank归档允许无ProjectVersion，导出/导入同格式且保留安全校验。

手动来源新增可选baseTemplate引用，GUI默认空白且必须选定有效基础。基于已有模板复制工程文件、资产及meta、UPM清单，VPM保留完整锁图和附加字段，按用户填写追加或覆盖直接依赖。固定Editor随基础继承；结果为独立归档。Core接受事务冻结基础内容并保护未终结任务引用，删除基础后仍可恢复执行，终态释放引用。分页、草稿、失效修订与重复名称均以既有规则处理。

没有新生产依赖、RPC方法/能力/权限、SQL表/列、State/Config版本。新增可选请求与journal字段；无基础字段的现有请求含义不变。新内置Blank可在Core正常启动播种目录时登记，本次未实际升级运行profile。

## 实际验证

证据目录：`target/m7-template-bases-20261109/`。最终源码/测试/合同摘要见`final-inputs.json`，所有计数按不同用例，不累加重复轮次。

| 命令/范围 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test -p alcomd-vpm project_create --lib` | 9通过 | vpm-tests-complete.log |
| `cargo test -p alcomd-store project_ --lib` | 15通过 | store-tests-complete.log |
| `cargo test -p alcomd-protocol --test schema_contract` | 48通过 | protocol-tests.log |
| `cargo test -p alcomd-application --lib` | 80通过 | application-tests.log |
| `cargo test -p alcomd --lib project_create_rpc::ipc_tests -- --nocapture` | 1通过 | ipc-tests-complete.log |

以上合计**153项Rust测试通过**，无忽略。覆盖真实归档展开、三个空白Editor版本、四个既有SDK模板、用户模板派生链、逐文件保留、完整VPM锁图、过期/版本拒绝、基础删除后的冻结/恢复/幂等及引用释放。私有Windows命名管道使用生产dispatcher和隔离Store、目录及任务，不连接现有Core。

- `cargo clippy -p alcomd -p alcomd-application -p alcomd-protocol -p alcomd-store -p alcomd-vpm --all-targets -- -D warnings`通过（clippy-final.log）。
- 全工作区`npm run check`与`npm run build`通过（frontend-check-complete.log、build-complete.log），含TypeScript、Material控件/文案/图标边界。保留既有大于500kB的bundle提示。
- `npm run test:browser --workspace @alcomd/gui -- m7-template-dialogs.spec.ts m7-template-lifecycle.spec.ts m7-template-pagination.spec.ts m7-template-operations.spec.ts m7-project-create-restore.spec.ts --workers=1 --reporter=line`最终**32项全部通过**（browser-complete.log）。覆盖空白默认/未安装Editor、固定基础、分页失败重试、修订变化保留草稿、原三种来源、导出/导入、只读内置及未知结果重放。标签补齐后的最终轮与此前32项复验为同一批不同用例，只计32项。
- 已实际查看中文1280px浅色、640px深色的创建模板截图，核对可见基础标签、选择内容、固定Unity版本、ID/名称、依赖与保存按钮。截图在GUI的test-results对应template-base路径；不作为原生窗口签收。
- `cargo fmt --all -- --check`、`cargo xtask check`与`git diff --check`均通过，见`final-gates.log`。

## 失败与修正

- 首次VPM轮7通过/1失败（vpm-tests.log）：新增实际创建回归发现空白最小PlayerSettings缺GUID占位；独立脚手架增加占位后8项通过，再增加完整锁图回归后9项通过。
- 首次Store轮13通过/2失败（store-tests.log）：新增夹具使用了非JSON请求指纹，被已有SQL约束正确拒绝；改为合法JSON，保持原校验，最终15项通过。
- 首次浏览器29项中28通过/1失败（browser.log）：批量调整去掉No template后的选择器计数时，误改资源表第二页行数为100；恢复原101行断言。后续新增修订/中文外观用例，32项通过；最后标签补齐后再次32项通过。
- 首次Clippy指出新增测试的一处多余借用（clippy.log），已按类型签名修正，严格检查通过。
- 初次`--bin alcomd project_create_ipc_tests`选错目标，运行0项（ipc-tests.log），不计验证。改用库内实际模块后私有管道1项通过，最终存储引用校验后再次1项通过。

## 范围与限制

本项未运行全工作区Rust测试或三平台Hosted CI/CodeQL，使用定向受影响模块与既有合成客户端。未连接真实项目、操作Unity、升级运行Core/GUI或真实profile。变更未提交/推送；M7/H7不扩大。之前预取6份源码/测试摘要在收尾复核，保持不变。
