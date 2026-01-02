# 网络流量统计重置功能指南

## 概述

本项目的流量统计功能已重构为**按月重置**模式，并支持**流量校准**和**配置热重载**。

---

## 主要功能

### 1. 按月重置流量统计

流量统计会在每月指定日期自动重置，默认为每月 1 号。

**配置方式**：
```bash
# 设置每月 5 号重置流量
./komari-monitor-rs --reset-day 5

# 设置每月最后一天重置（例如 31 号，2 月会自动使用 28/29 号）
./komari-monitor-rs --reset-day 31
```

**注意事项**：
- `reset-day` 范围为 1-31
- 如果当月没有指定日期（如 2 月 31 日），会自动使用当月最后一天
  - 设置 31 号：1/3/5/7/8/10/12 月在 31 号重置，其他月在最后一天重置
  - 设置 30 号：2 月会在 28/29 号重置（闰年 29 号，平年 28 号）
- 重置时会记录当前系统流量作为新的基准值
- 重置操作会立即保存到配置文件

---

### 2. 流量校准

允许用户设置基准流量值，使统计数据与 VPS 服务商的统计保持一致。

**使用场景**：
- VPS 服务商显示你本月已使用 10GB 上传、20GB 下载
- 你希望监控程序显示相同的数值以便对齐

**配置方式**：
```bash
# 设置流量校准值（单位：字节）
./komari-monitor-rs \
  --calibration-tx 10737418240 \    # 10GB 上传
  --calibration-rx 21474836480      # 20GB 下载
```

**或者通过编辑配置文件**：
```bash
# 编辑配置文件（Linux root）
sudo nano /etc/komari-network.conf

# 修改以下字段
calibration_tx=10737418240
calibration_rx=21474836480
```

**计算公式**：
- 最终显示流量 = 本周期实际流量 + 校准值
- 校准值会持久化保存，重启不丢失

---

### 3. 配置文件热重载

程序每次采样时会自动检测配置文件是否变化，无需重启程序即可应用新配置。

**支持热重载的配置项**：
- `calibration_tx` / `calibration_rx` - 流量校准值
- `reset_day` - 重置日期
- `network_interval` - 采样间隔

**操作步骤**：
1. 直接编辑配置文件
2. 保存文件
3. 等待最多 `network_interval` 秒（默认 10 秒）
4. 程序自动检测并应用新配置

**示例**：
```bash
# 1. 查看当前配置文件位置
# Linux (root): /etc/komari-network.conf
# Linux (普通用户): ~/.config/komari-network.conf
# Windows: C:\komari-network.conf

# 2. 编辑配置文件
sudo nano /etc/komari-network.conf

# 3. 修改校准值
calibration_tx=5368709120   # 5GB
calibration_rx=10737418240  # 10GB

# 4. 保存并退出，程序会在 10 秒内自动重载
```

---

## 配置文件格式

配置文件采用 `key=value` 格式：

```ini
# 网络统计配置
disable_network_statistics=false
network_interval=10
reset_day=1
calibration_tx=0
calibration_rx=0
network_save_path=/etc/komari-network.conf

# 系统信息（由程序自动管理）
boot_id=12345678-1234-1234-1234-123456789abc
source_tx=1234567890
source_rx=9876543210
latest_tx=1000000
latest_rx=2000000
last_reset_month=1
```

**字段说明**：

| 字段 | 类型 | 说明 | 是否可手动修改 |
|------|------|------|----------------|
| `disable_network_statistics` | bool | 是否禁用网络统计 | ⚠️ 建议通过命令行参数 |
| `network_interval` | u32 | 采样间隔（秒） | ✅ 可修改 |
| `reset_day` | u8 | 每月重置日期（1-31） | ✅ 可修改 |
| `calibration_tx` | u64 | 上传流量校准值（字节） | ✅ 可修改 |
| `calibration_rx` | u64 | 下载流量校准值（字节） | ✅ 可修改 |
| `network_save_path` | string | 配置文件路径 | ⚠️ 不建议修改 |
| `boot_id` | string | 系统启动 ID | ❌ 程序管理 |
| `source_tx/rx` | u64 | 基准流量 | ❌ 程序管理 |
| `latest_tx/rx` | u64 | 本周期流量 | ❌ 程序管理 |
| `last_reset_month` | u8 | 上次重置月份 | ❌ 程序管理 |

---

## 工作原理

### 流量计算逻辑

```
当前周期流量 = 系统累计流量 - 基准流量 (source_tx/rx)
最终显示流量 = 当前周期流量 + 校准值 (calibration_tx/rx)
```

### 重置机制

**按月重置触发条件**：
- 当前月份 ≠ 上次重置月份（`last_reset_month`）
- 当前日期 ≥ 设置的重置日期（`reset_day`）

**重置时的操作**：
1. 记录当前系统流量作为新的 `source_tx/rx`
2. 清零 `latest_tx/rx`
3. 更新 `last_reset_month` 为当前月份
4. 立即保存到配置文件

**示例时间线**：
```
# 设置 reset_day=1（每月 1 号重置）
1月1日  - 自动重置，source=当前系统流量，latest=0
1月2日 - 1月31日 - 累计流量到 latest
2月1日  - 自动重置，source=当前系统流量，latest=0（重新开始）

# 设置 reset_day=31（每月最后一天重置）
1月31日 - 自动重置（1月有31天）
2月28日 - 自动重置（2月平年只有28天，自动使用最后一天）
2月29日 - 自动重置（2月闰年有29天，自动使用最后一天）
3月31日 - 自动重置（3月有31天）
4月30日 - 自动重置（4月只有30天，自动使用最后一天）
```

### 系统重启处理

- **Linux**: 通过 `/proc/sys/kernel/random/boot_id` 检测重启
- **Windows**: 每次启动都认为是重启

重启时的处理：
- 将 `latest_tx/rx` 合并到 `source_tx/rx`
- 重置 `latest_tx/rx` 为 0
- 保留 `last_reset_month` 和校准值

---

## 实用技巧

### 1. 快速对齐 VPS 服务商流量

假设服务商显示：
- 本月上传：15.5 GB
- 本月下载：42.3 GB

```bash
# 1. 计算字节数（可使用在线计算器）
# 15.5 GB = 16642998272 bytes
# 42.3 GB = 45401866649 bytes

# 2. 编辑配置文件
sudo nano /etc/komari-network.conf

# 3. 修改校准值
calibration_tx=16642998272
calibration_rx=45401866649

# 4. 保存后等待 10 秒，流量自动对齐
```

### 2. 手动触发重置

如果需要手动重置流量统计（不等待月度周期）：

```bash
# 方法 1: 编辑配置文件，修改 last_reset_month
sudo nano /etc/komari-network.conf
# 将 last_reset_month 改为上个月的数字（1-12）
# 例如当前是 5 月，改为 4

# 方法 2: 删除配置文件重新初始化
sudo rm /etc/komari-network.conf
sudo systemctl restart komari-monitor
```

### 3. GB/TB 转换表

| 单位 | 字节数 | 示例 |
|------|--------|------|
| 1 GB | 1073741824 | `calibration_tx=1073741824` |
| 10 GB | 10737418240 | `calibration_tx=10737418240` |
| 100 GB | 107374182400 | `calibration_tx=107374182400` |
| 1 TB | 1099511627776 | `calibration_tx=1099511627776` |

**计算公式**：
```
字节数 = GB × 1024 × 1024 × 1024
```

---

## 命令行参数完整列表

```bash
./komari-monitor-rs \
  --disable-network-statistics       # 禁用网络统计
  --network-interval 10              # 采样间隔（秒，默认 10）
  --reset-day 1                      # 每月重置日期（1-31，默认 1，超过当月天数时使用最后一天）
  --calibration-tx 0                 # 上传校准值（字节，默认 0）
  --calibration-rx 0                 # 下载校准值（字节，默认 0）
  --network-save-path /path/to/file  # 配置文件路径（可选）
```

---

## 日志信息

程序运行时会输出相关日志：

**初始化**：
```
Network traffic info file is empty, possibly first run or save path changed, created new file
```

**配置变更**：
```
Network configuration changed, applying new configuration while preserving traffic data
Configuration reloaded successfully
```

**系统重启**：
```
System reboot detected, merging traffic data
```

**月度重置**：
```
Monthly traffic reset triggered (configured day: 31, effective day: 28, current month: 2)
Traffic statistics reset completed
```

**说明**：
- `configured day`: 用户配置的重置日期
- `effective day`: 实际生效的重置日期（当月没有配置日期时，使用月末）
- `current month`: 当前月份（1-12）

**配置热重载**：
```
Configuration file changed detected, reloading configuration
Configuration reloaded successfully
```

---

## 故障排查

### Q: 配置修改后没有生效？

1. 检查配置文件语法是否正确（`key=value` 格式，无空格）
2. 确认文件权限是否正确（`chmod 644 /etc/komari-network.conf`）
3. 等待至少一个 `network_interval` 周期（默认 10 秒）
4. 查看程序日志确认是否有错误信息

### Q: 流量统计显示负数？

这通常发生在：
- 系统重启后网络接口计数器重置
- 网络接口被重新配置

解决方法：删除配置文件重新初始化
```bash
sudo rm /etc/komari-network.conf
sudo systemctl restart komari-monitor
```

### Q: 如何验证配置是否正确？

```bash
# 查看当前配置文件内容
cat /etc/komari-network.conf

# 检查程序日志
journalctl -u komari-monitor -f
```

---

## 升级说明

**⚠️ 注意**：本次重构不兼容旧版配置文件。

如果从旧版本升级：
1. 备份旧配置文件（如果需要）
2. 删除旧配置文件
3. 启动新版本程序自动创建新配置

```bash
# 备份旧配置（可选）
sudo cp /etc/komari-network.conf /etc/komari-network.conf.backup

# 删除旧配置
sudo rm /etc/komari-network.conf

# 启动程序
sudo systemctl restart komari-monitor
```

---

## 技术细节

- **时间处理**: 使用 `time` crate，优先使用本地时间，失败时回退到 UTC
- **持久化**: 每 10 个采样周期写入一次磁盘，重置时立即写入
- **配置检测**: 每个采样周期检查一次配置文件变化
- **原子性**: 配置文件使用覆盖写入确保原子性

---

## 贡献

如有问题或建议，欢迎提交 Issue 或 Pull Request。
