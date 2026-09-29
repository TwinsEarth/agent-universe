//! 沙箱 Kubernetes API（v2.8.0，声明 + 清单生成）
//!
//! 面向企业平台 / 基础设施团队：把 Agent Sandbox 建模为 Kubernetes 自定义资源
//! （CRD `AgentSandbox`），由一个 operator 协调为 MicroVM/进程沙箱，
//! 从而纳入现有 K8s 的资源管理、安全治理和运维流程。
//!
//! 本模块提供 CRD 清单与示例实例的 **YAML 生成**（纯函数、可测试）。
//! 当前节点环境不内置 Kubernetes 集群，故只生成可 `kubectl apply` 的清单，
//! 不实际连接集群（operator 的协调循环见 `docs/sandbox/kubernetes-api.md`）。

/// CRD 的 plural / group / kind
pub const CRD_GROUP: &str = "agent.twinsearth.dev";
pub const CRD_PLURAL: &str = "agentsandboxes";
pub const CRD_KIND: &str = "AgentSandbox";
pub const CRD_VERSION: &str = "v1alpha1";

/// 生成 CRD 定义清单（CustomResourceDefinition）
pub fn crd_manifest() -> String {
    format!(
        "apiVersion: apiextensions.k8s.io/v1
kind: CustomResourceDefinition
metadata:
  name: {plural}.{group}
spec:
  group: {group}
  scope: Namespaced
  names:
    kind: {kind}
    plural: {plural}
    singular: agentsandbox
    shortNames:
      - asb
  versions:
    - name: {version}
      served: true
      storage: true
      schema:
        openAPIV3Schema:
          type: object
          properties:
            spec:
              type: object
              required: [template]
              properties:
                template:
                  type: string
                  description: 沙箱模板，如 process:generic / microvm:python
                isolation:
                  type: string
                  enum: [process, container-shared-kernel, microvm]
                resources:
                  type: object
                  properties:
                    cpuMillis: {{ type: integer }}
                    memMb: {{ type: integer }}
                    diskMb: {{ type: integer }}
                    timeoutMs: {{ type: integer }}
                network:
                  type: object
                  properties:
                    allowEgress: {{ type: boolean }}
                    denyRawIp: {{ type: boolean }}
                    egressAllowlist:
                      type: array
                      items: {{ type: string }}
            status:
              type: object
              properties:
                phase:
                  type: string
                  enum: [pending, creating, running, paused, stopped, failed]
      subresources:
        status: {{}}
      additionalPrinterColumns:
        - name: Phase
          type: string
          jsonPath: .status.phase
        - name: Isolation
          type: string
          jsonPath: .spec.isolation
        - name: Age
          type: date
          jsonPath: .metadata.creationTimestamp
",
        plural = CRD_PLURAL,
        group = CRD_GROUP,
        kind = CRD_KIND,
        version = CRD_VERSION,
    )
}

/// 生成一个示例 AgentSandbox 实例
pub fn example_instance(name: &str, namespace: &str) -> String {
    format!(
        "apiVersion: {group}/{version}
kind: {kind}
metadata:
  name: {name}
  namespace: {namespace}
spec:
  template: process:generic
  isolation: process
  resources:
    cpuMillis: 1000
    memMb: 256
    diskMb: 128
    timeoutMs: 30000
  network:
    allowEgress: false
    denyRawIp: true
    egressAllowlist: []
",
        group = CRD_GROUP,
        version = CRD_VERSION,
        kind = CRD_KIND,
        name = name,
        namespace = namespace,
    )
}

/// operator 协调逻辑说明（供文档与运维参考）
pub fn reconcile_outline() -> Vec<&'static str> {
    vec![
        "1. watch AgentSandbox 增删改",
        "2. 不存在对应 Pod/沙箱时：按 template + resources 创建（MicroVM/进程）",
        "3. 把 spec.network 渲染为 NetworkPolicy，spec.resources 渲染为配额",
        "4. 回写 status.phase；失败回写 failed 并保留事件",
        "5. 删除时：先优雅停止、清理临时盘，再移除 finalizer",
    ]
}
