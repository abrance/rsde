# Debian12 系统 DNS 配置 - K8s CoreDNS 优先级配置

## ✅ 配置完成

已成功配置系统 DNS，让 CoreDNS 优先处理 K8s 集群域名解析。

## 📊 当前系统 DNS 架构

```
宿主机应用
    ↓
127.0.0.1:53 (dnsmasq 本地缓存)
    ↓
    ├── *.cluster.local → 127.0.0.1:30053 (K8s CoreDNS)
    ├── *.svc.cluster.local → 127.0.0.1:30053 (K8s CoreDNS)  
    └── 其他域名 → 182.254.116.116 等上游 DNS
```

## 🔧 已完成的配置步骤

### 1. dnsmasq 配置（K8s 域名转发）

**配置文件**: `/etc/dnsmasq.d/k8s.conf`

```conf
# K8s CoreDNS 配置
# 将 .cluster.local 域名请求转发到 CoreDNS
server=/cluster.local/127.0.0.1#30053

# 为所有 .svc.cluster.local 域名请求转发到 CoreDNS
server=/svc.cluster.local/127.0.0.1#30053
```

### 2. 系统 DNS 配置

**配置文件**: `/etc/resolv.conf`

```conf
# Use local dnsmasq for K8s DNS
nameserver 127.0.0.1
# Fallback DNS
nameserver 182.254.116.116
nameserver 114.114.114.114
```

### 3. NetworkManager 配置

**配置文件**: `/etc/NetworkManager/conf.d/dns.conf`

```conf
[main]
dns=dnsmasq
rc-manager=unmanaged
```

### 4. 保护 resolv.conf 不被覆盖

已设置文件为不可修改：
```bash
sudo chattr +i /etc/resolv.conf
```

## ✅ 验证测试

### 测试 K8s 集群服务解析

```bash
# Kafka 服务解析
$ dig test-kafka.bkbase-test.svc.cluster.local +short
10.43.210.177

# Kubernetes API 服务
$ dig kubernetes.default.svc.cluster.local +short
10.43.0.1

# CoreDNS 服务
$ dig kube-dns.kube-system.svc.cluster.local +short
10.43.0.10
```

### 测试外部域名解析（确保不影响正常上网）

```bash
$ dig google.com +short
8.7.198.46  # 正常解析
```

### 查看 DNS 查询使用的服务器

```bash
$ dig test-kafka.bkbase-test.svc.cluster.local | grep SERVER
;; SERVER: 127.0.0.1#53(127.0.0.1) (UDP)
```

## 🎯 DNS 解析优先级

1. **K8s 集群域名** (`*.cluster.local`, `*.svc.cluster.local`) → **CoreDNS (优先级最高)**
2. **其他域名** → 上游 DNS (182.254.116.116, 114.114.114.114)
3. **Clash 代理** → 不影响 DNS 解析优先级

## 📝 在应用中使用

### Python 示例（直接使用域名）

```python
from kafka import KafkaProducer

# 现在可以直接使用 K8s 服务域名！
producer = KafkaProducer(
    bootstrap_servers=['test-kafka.bkbase-test.svc.cluster.local:9092']
)

# 或使用简短形式（如果在同一命名空间）
# bootstrap_servers=['test-kafka:9092']
```

### Rust 示例（直接使用域名）

```rust
use rdkafka::config::ClientConfig;
use rdkafka::producer::FutureProducer;

let producer: FutureProducer = ClientConfig::new()
    .set("bootstrap.servers", "test-kafka.bkbase-test.svc.cluster.local:9092")
    .set("message.timeout.ms", "5000")
    .create()
    .expect("Producer creation error");
```

### 命令行工具测试

```bash
# 使用 curl 访问 K8s 服务
curl http://test-kafka.bkbase-test.svc.cluster.local:9092

# 使用 ping 测试（需要服务支持 ICMP）
ping test-kafka.bkbase-test.svc.cluster.local

# 使用 telnet 测试端口
telnet test-kafka.bkbase-test.svc.cluster.local 9092
```

## 🔍 故障排查

### 1. 检查 dnsmasq 是否正常运行

```bash
sudo systemctl status dnsmasq
```

### 2. 查看 dnsmasq 日志

```bash
sudo journalctl -u dnsmasq -f
```

### 3. 检查 DNS 配置是否生效

```bash
# 查看 dnsmasq 使用的上游 DNS
sudo cat /var/log/syslog | grep dnsmasq | tail -20

# 或查看启动日志
sudo systemctl status dnsmasq | grep "using nameserver"
```

### 4. 测试 DNS 解析路径

```bash
# 详细查询过程
dig test-kafka.bkbase-test.svc.cluster.local +trace

# 使用特定 DNS 服务器测试
dig @127.0.0.1 test-kafka.bkbase-test.svc.cluster.local
dig @127.0.0.1 -p 30053 test-kafka.bkbase-test.svc.cluster.local
```

### 5. 如果 resolv.conf 被覆盖

```bash
# 检查文件属性
lsattr /etc/resolv.conf

# 如果需要修改，先解除保护
sudo chattr -i /etc/resolv.conf

# 修改后重新保护
echo -e "nameserver 127.0.0.1\nnameserver 182.254.116.116" | sudo tee /etc/resolv.conf
sudo chattr +i /etc/resolv.conf
```

### 6. 重启所有相关服务

```bash
# 重启 dnsmasq
sudo systemctl restart dnsmasq

# 重启 NetworkManager（可选）
sudo systemctl restart NetworkManager

# 检查 CoreDNS 是否正常
kubectl get pods -n kube-system -l k8s-app=kube-dns
```

## 📋 服务管理命令

### dnsmasq 服务

```bash
# 启动
sudo systemctl start dnsmasq

# 停止
sudo systemctl stop dnsmasq

# 重启
sudo systemctl restart dnsmasq

# 查看状态
sudo systemctl status dnsmasq

# 查看配置
cat /etc/dnsmasq.d/k8s.conf
```

### CoreDNS 服务

```bash
# 查看 CoreDNS Pod
kubectl get pods -n kube-system -l k8s-app=kube-dns

# 查看 CoreDNS 日志
kubectl logs -n kube-system -l k8s-app=kube-dns --tail=50

# 查看外部暴露的服务
kubectl get svc -n kube-system coredns-external

# 测试直接连接 CoreDNS
dig @localhost -p 30053 kubernetes.default.svc.cluster.local
```

## ⚙️ 高级配置

### 添加更多自定义域名转发

如果你想为特定的命名空间或服务配置 DNS，可以编辑 `/etc/dnsmasq.d/k8s.conf`：

```bash
# 编辑配置
sudo nano /etc/dnsmasq.d/k8s.conf

# 添加类似这样的规则：
# server=/bkbase-test.svc.cluster.local/127.0.0.1#30053
# server=/default.svc.cluster.local/127.0.0.1#30053
# server=/monitoring.svc.cluster.local/127.0.0.1#30053

# 重启 dnsmasq
sudo systemctl restart dnsmasq
```

### 添加本地 DNS 记录

```bash
# 编辑 /etc/hosts 或创建 dnsmasq 配置
sudo nano /etc/dnsmasq.d/local-hosts.conf

# 添加内容：
# address=/my-local-service.local/192.168.1.100
```

## 🎉 优势总结

✅ **无缝服务发现**: 宿主机程序可以直接使用 K8s 服务域名  
✅ **高优先级**: K8s 域名优先通过 CoreDNS 解析  
✅ **不影响外网**: 外部域名正常通过上游 DNS 解析  
✅ **兼容 Clash**: 不影响 Clash 代理功能  
✅ **持久化配置**: 重启后配置依然生效  
✅ **DNS 缓存**: dnsmasq 提供本地 DNS 缓存，提升解析速度  

## 📌 重要提示

⚠️ **关于 .local 域名警告**  
你可能会看到这样的警告：
```
WARNING: .local is reserved for Multicast DNS
```
这是因为 `.local` 域名被保留用于 mDNS (Multicast DNS)，但 K8s 使用 `.cluster.local` 是标准做法，可以忽略这个警告。

💡 **配置文件位置汇总**
- dnsmasq K8s 配置: `/etc/dnsmasq.d/k8s.conf`
- 系统 DNS 配置: `/etc/resolv.conf`
- NetworkManager 配置: `/etc/NetworkManager/conf.d/dns.conf`
- CoreDNS K8s 配置: 通过 kubectl 管理

🔧 **如果需要临时禁用**
```bash
# 临时移除 K8s DNS 配置
sudo mv /etc/dnsmasq.d/k8s.conf /etc/dnsmasq.d/k8s.conf.bak
sudo systemctl restart dnsmasq

# 恢复配置
sudo mv /etc/dnsmasq.d/k8s.conf.bak /etc/dnsmasq.d/k8s.conf
sudo systemctl restart dnsmasq
```
