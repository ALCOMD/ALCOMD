# Extension Host Capabilities v1

状态：Draft

后台扩展不得获得裸 OS 句柄。Extension Host 暴露窄能力：

```text
host.network.request
host.notification.send
host.external-config.plan
host.external-config.apply
host.discord-presence.connect
host.discord-presence.update
host.discord-presence.clear
```

每个能力必须定义：

- 公开权限。
- 输入和输出 Schema。
- 资源范围。
- 超时、配额与并发。
- 日志脱敏。
- 撤销语义。
- 崩溃与取消行为。
- 平台差异。
