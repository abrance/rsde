# K3s CoreDNS 宿主机访问配置

## 已完成配置

已成功将 CoreDNS 通过 NodePort 方式暴露到宿主机。

### 服务信息
- **服务名称**: `coredns-external`
- **命名空间**: `kube-system`
- **类型**: NodePort
- **宿主机端口**: `30053` (UDP/TCP)
- **集群内部 IP**: `10.43.185.36`

## 使用方法

### 1. 在宿主机程序中使用 DNS

在你的宿主机程序中，将 DNS 服务器设置为：
```
localhost:30053
# 或
127.0.0.1:30053
```

### 2. 命令行测试

#### 使用 dig 测试
```bash
# 解析 Kubernetes API 服务
dig @localhost -p 30053 kubernetes.default.svc.cluster.local

# 解析你的 Kafka 服务
dig @localhost -p 30053 test-kafka.bkbase-test.svc.cluster.local

# 简短输出
dig @localhost -p 30053 test-kafka.bkbase-test.svc.cluster.local +short
```

#### 使用 nslookup 测试
```bash
nslookup test-kafka.bkbase-test.svc.cluster.local localhost -port=30053
```

### 3. 在应用程序中配置

#### Python 示例（使用 kafka-python）
```python
from kafka import KafkaConsumer, KafkaProducer
import socket

# 方案 1: 直接使用 ClusterIP (如果网络可达)
bootstrap_servers = ['10.43.210.177:9092']

# 方案 2: 使用 NodePort 服务
bootstrap_servers = ['localhost:30092']  # kafka-external 的 NodePort

# 方案 3: 使用 LoadBalancer
bootstrap_servers = ['10.45.53.44:31096']  # kafka-loadbalancer 的外部 IP
```

#### Rust 示例（rdkafka）
```rust
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};

let producer: FutureProducer = ClientConfig::new()
    .set("bootstrap.servers", "10.43.210.177:9092")  // 使用 ClusterIP
    // 或使用 NodePort
    // .set("bootstrap.servers", "localhost:30092")
    .set("message.timeout.ms", "5000")
    .create()
    .expect("Producer creation error");
```

## 服务发现说明

### 集群内部服务 DNS 格式
```
<service-name>.<namespace>.svc.cluster.local
```

### 你的 Kafka 服务解析示例
```bash
# 完整域名
test-kafka.bkbase-test.svc.cluster.local -> 10.43.210.177

# 简短形式（在同一命名空间内）
test-kafka -> 10.43.210.177
```

## 当前集群服务列表

### bkbase-test 命名空间
- **test-kafka** (ClusterIP: 10.43.210.177:9092) - Kafka 服务
- **kafka-external** (NodePort: 30092) - Kafka NodePort 访问
- **kafka-loadbalancer** (LoadBalancer: 10.45.53.44:31096) - Kafka LoadBalancer 访问

### 宿主机访问 Kafka 的三种方式

#### 方式 1: 通过 NodePort (推荐用于开发)
```
localhost:30092
```

#### 方式 2: 通过 LoadBalancer (推荐用于生产)
```
10.45.53.44:31096
```

#### 方式 3: 通过 ClusterIP + DNS (需要网络可达)
```
test-kafka.bkbase-test.svc.cluster.local:9092
# 解析后: 10.43.210.177:9092
```

## 网络配置说明

### K3s 默认网络范围
- **Service CIDR**: `10.43.0.0/16`
- **Pod CIDR**: 通常是 `10.42.0.0/16`

### 访问集群服务的网络要求

1. **直接访问 ClusterIP**: 需要宿主机能路由到 Service CIDR (10.43.0.0/16)
2. **使用 NodePort**: 直接通过 localhost 或宿主机 IP 访问
3. **使用 LoadBalancer**: 通过 MetalLB 分配的外部 IP 访问

## 配置文件

CoreDNS 外部服务配置文件: `coredns-nodeport.yaml`

```yaml
apiVersion: v1
kind: Service
metadata:
  name: coredns-external
  namespace: kube-system
spec:
  type: NodePort
  selector:
    k8s-app: kube-dns
  ports:
  - name: dns-udp
    protocol: UDP
    port: 53
    targetPort: 53
    nodePort: 30053
  - name: dns-tcp
    protocol: TCP
    port: 53
    targetPort: 53
    nodePort: 30053
```

## 故障排查

### 测试 DNS 连接
```bash
# 测试 UDP
nc -u -v localhost 30053

# 测试 TCP
nc -v localhost 30053
```

### 查看 CoreDNS 日志
```bash
kubectl logs -n kube-system -l k8s-app=kube-dns --tail=50
```

### 验证服务状态
```bash
kubectl get svc -n kube-system coredns-external
kubectl get pods -n kube-system -l k8s-app=kube-dns
```

## 重要提示

⚠️ **端口 30053 注意事项**:
- 标准 DNS 使用 53 端口，这里使用 30053 避免与系统 DNS 冲突
- 某些 DNS 客户端库可能不支持非标准端口
- 如需使用标准 53 端口，可能需要 root 权限或使用 HostNetwork

💡 **推荐做法**:
- **开发环境**: 直接使用 NodePort 服务（如 kafka-external:30092）
- **生产环境**: 使用 LoadBalancer 服务（如 10.45.53.44:31096）
- **服务发现**: 对于需要动态服务发现的场景，使用 DNS 解析

🔧 **宿主机程序连接 Kafka 最佳实践**:
1. 优先使用 NodePort (localhost:30092) - 最简单
2. 其次使用 LoadBalancer IP (10.45.53.44:31096) - 更灵活
3. 避免直接使用 ClusterIP，除非确认网络已打通
