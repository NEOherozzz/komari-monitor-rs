# 网络流量统计功能变更日志

## [Unreleased] - 2026-01-02

### ✨ 新增功能

- **按月重置**: 流量统计在每月固定日期（可配置）自动重置，默认为每月 1 号
  - 新增 `--reset-day` 参数（范围 1-31）
  - 智能处理月末日期：如果当月没有指定日期（如 2 月 31 日），自动使用当月最后一天
  - 自动检测月份变化并重置统计

- **流量校准**: 支持设置基准流量值，与 VPS 服务商统计对齐
  - 新增 `--calibration-tx` 参数（上传校准值，字节）
  - 新增 `--calibration-rx` 参数（下载校准值，字节）
  - 最终显示流量 = 本周期流量 + 校准值

- **配置热重载**: 支持运行时修改配置文件，无需重启程序
  - 每个采样周期自动检测配置文件变化
  - 保留流量数据的同时应用新配置
  - 最长延迟为 `network_interval` 秒（默认 10 秒）

### 🔄 变更

- **配置变更处理优化**: 配置参数改变时不再清空流量数据，而是保留数据并应用新配置
- **系统重启处理改进**: Linux 通过 boot_id 精确检测重启，Windows 每次启动自动合并流量
- **磁盘写入策略**: 固定每 10 个采样周期写入一次磁盘（移除 `network_interval_number` 配置）

### ❌ 移除

- `--network-duration` 参数 - 不再使用周期计数模式
- `--network-interval-number` 参数 - 固定为每 10 次采样写入一次
- `counter` 字段 - 不再需要倒计时

### 📝 配置文件格式变更

**新增字段**:
```ini
reset_day=1              # 每月重置日期
calibration_tx=0         # 上传校准值（字节）
calibration_rx=0         # 下载校准值（字节）
last_reset_month=1       # 上次重置月份（程序管理）
```

**移除字段**:
```ini
network_duration         # 已移除
network_interval_number  # 已移除
counter                  # 已移除
```

### ⚠️ 破坏性变更

- **不向前兼容**: 旧版配置文件无法直接使用，需要删除重建
- **升级步骤**:
  1. 备份旧配置（可选）: `cp /etc/komari-network.conf /etc/komari-network.conf.old`
  2. 删除旧配置: `rm /etc/komari-network.conf`
  3. 重启程序自动创建新配置

### 📚 文档

- 新增 `NETWORK_RESET_GUIDE.md` - 详细使用指南
- 新增 `komari-network.conf.example` - 示例配置文件
- 新增 `.claude/REFACTORING_SUMMARY.md` - 技术重构总结

### 🐛 修复

- 修复配置变更时不必要的 3 秒等待
- 改进错误处理和日志输出

---

## 快速开始示例

### 每月 5 号重置流量
```bash
./komari-monitor-rs --reset-day 5
```

### 每月最后一天重置流量
```bash
./komari-monitor-rs --reset-day 31
# 1/3/5/7/8/10/12 月在 31 日重置
# 4/6/9/11 月在 30 日重置
# 2 月在 28/29 日重置（平年/闰年）
```

### 对齐服务商流量（假设已使用 50GB 上传，100GB 下载）
```bash
./komari-monitor-rs \
  --calibration-tx 53687091200 \
  --calibration-rx 107374182400
```

### 运行时修改校准值
```bash
# 编辑配置文件
sudo nano /etc/komari-network.conf

# 修改以下行
calibration_tx=53687091200
calibration_rx=107374182400

# 保存后 10 秒内自动生效，无需重启
```

---

## 迁移指南

### 从旧版本迁移

如果你之前使用了以下参数：
```bash
--network-duration 864000 \
--network-interval-number 10
```

新版本中应移除这些参数，并根据需要添加：
```bash
--reset-day 1 \              # 每月 1 号重置
--calibration-tx 0 \         # 初始无需校准
--calibration-rx 0
```

### 配置文件迁移

**旧配置** (会被拒绝):
```ini
network_duration=864000
counter=86400
```

**新配置** (自动生成):
```ini
reset_day=1
last_reset_month=1
```

---

## 技术细节

- **时间处理**: 使用 `time` crate 的 `OffsetDateTime`
- **月份检测**: 对比 `last_reset_month` 和当前月份
- **重置触发**: 月份变化 + 达到重置日期
- **流量计算**: `total = (current - source) + calibration`

---

## 贡献者

感谢所有参与此次重构的贡献者！

---

**完整文档**: 查看 [NETWORK_RESET_GUIDE.md](NETWORK_RESET_GUIDE.md) 了解更多详情。
