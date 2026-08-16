# 开放决策

以下事项必须由项目所有者确认，Codex 不得自行拍板。

## O-003 4.0.0 平台技术基线

A-017 与 A-024 已批准三个发布平台和四种主要发行格式：

1. Windows x86_64 Inno Setup EXE。
2. macOS arm64 DMG。
3. Linux x86_64 AppImage。
4. Linux amd64 DEB。

Windows 当前用户安装与所有用户安装是同一个 Inno Setup EXE 的两种模式，不是两个发行资产。

仍需冻结：

- Windows 最低版本与 WebView2 分发策略。
- Linux 最低运行发行版/glibc 与发行资产构建基线。
- macOS 最低 deployment target。
- macOS GUI 黑盒测试采用真实机器自动化还是人工签收。

这些值不能由当前 CI runner 版本自动推导。
