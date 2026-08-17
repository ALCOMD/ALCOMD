# VPM 生态兼容基线

状态：M-1 公开格式、生态兼容范围与验收合同已冻结

## 定位

ALCOMD v4 的 VPM 能力是独立实现。`source-lock.toml` 冻结一个 vrc-get commit，
仅用于让行为观察、Fixture 和差异测试可复现；这不把 vrc-get / vrc-get-vpm 变成
代码上游，也不允许复制、Fork、包装、移植或改写其源码。

固定版本 vrc-get 的功能、安全、CLI 与错误行为由 `vrc-get.md` 单独建档；本文聚焦
不依赖某一实现的公开格式、生态兼容要求和跨实现 Fixture。两份基线必须同时保留。

## 审计领域

- 公开 VPM repository 与 package manifest 格式。
- `Packages/vpm-manifest.json` 的生态兼容行为。
- 依赖解析、版本选择、Unity 版本与包兼容性。
- 包下载、缓存、校验、解压、路径安全与安装事务。
- 用户仓库、用户包、模板和跨平台差异。
- ALCOMD3 v3 的用户可见行为与真实项目 Fixture。

## 实现边界

功能审计产生的是行为需求和测试用例，不产生源码复用许可。应用层只依赖
`alcomd-vpm` 门面，具体实现必须由 ALCOMD 项目独立完成。
