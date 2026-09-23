# Security Policy

## 报告安全漏洞

如果你发现安全漏洞，请**不要**创建公开 Issue。

请通过以下方式私下报告：

1. 发送邮件到 security@agent-universe.dev
2. 或通过 GitHub [Private Vulnerability Reporting](https://github.com/TwinsEarth/agent-universe/security/advisories/new)

## 响应时间

- 我们会在 **48 小时内** 确认收到报告
- 7 天内提供初步评估
- 确认漏洞后，我们将与你协调修复时间表

## 安全最佳实践

### 密钥管理

- 永远不要将私钥提交到代码仓库
- 使用环境变量或密钥管理服务
- 定期轮换密钥

### 依赖安全

- 定期更新依赖：`cargo update && pip list --outdated`
- 关注安全公告
- 最小化依赖数量

### 节点安全

- 只开放必要端口
- 使用防火墙限制访问
- 定期更新系统和依赖
- 监控异常行为
