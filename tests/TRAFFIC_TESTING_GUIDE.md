# 流量统计测试指南

本文档提供详细的测试方案，帮助你验证 Komari Monitor 的流量统计和重置功能是否正常工作。

## 测试环境准备

### 1. 确认程序已安装并运行

```bash
# 检查程序是否运行
sudo systemctl status komari-monitor

# 查看实时日志
sudo journalctl -u komari-monitor -f
```

### 2. 确认配置文件

```bash
# 查看配置文件（root 用户）
cat /etc/komari-agent.conf

# 查看配置文件（非 root 用户）
cat ~/.config/komari-agent.conf
```

重点检查以下配置项：
- `disable_network_statistics=false` (必须为 false)
- `network_interval=10` (数据保存间隔，建议测试时设为 10 秒)
- `reset_day=1` (月度重置日期，可以设置为任意 1-31 的值)

### 3. 定位运行时数据文件

```bash
# Root 用户
RUNTIME_DATA="/var/lib/komari-monitor/network-data.conf"

# 非 root 用户
RUNTIME_DATA="$HOME/.local/share/komari-monitor/network-data.conf"
```

## 测试方案

### 方法一：使用测试脚本（推荐）

我们提供了自动化测试脚本，简化测试流程：

```bash
# 添加执行权限
chmod +x tests/traffic_test.sh

# 查看帮助
./tests/traffic_test.sh help

# 查看当前运行时数据
./tests/traffic_test.sh view

# 测试系统重启场景
./tests/traffic_test.sh reboot

# 测试月度重置场景
./tests/traffic_test.sh monthly

# 测试组合场景（重启+月度重置）
./tests/traffic_test.sh combined

# 恢复备份数据
./tests/traffic_test.sh restore
```

### 方法二：手动测试

如果你想更深入理解测试过程，可以手动执行以下测试。

---

## 测试 1: 系统重启场景

**目的**: 验证程序在系统重启后能正确处理流量数据。

### 测试步骤

#### 1. 记录初始状态

```bash
# 查看当前运行时数据
cat $RUNTIME_DATA
```

记录以下关键字段：
- `boot_id`: 当前启动 ID
- `boot_source_tx/rx`: 本次启动的基准流量
- `current_boot_tx/rx`: 本次启动的累计流量增量
- `accumulated_tx/rx`: 历史累计流量
- `last_reset_month`: 上次重置月份

示例输出：
```
boot_id=a1b2c3d4-e5f6-7890-1234-567890abcdef
boot_source_tx=1000000000
boot_source_rx=5000000000
current_boot_tx=100000000
current_boot_rx=200000000
accumulated_tx=500000000
accumulated_rx=800000000
last_reset_month=12
```

#### 2. 模拟系统重启

**方式 A: 实际重启系统（最真实）**

```bash
sudo reboot
```

**方式 B: 修改 boot_id 模拟重启（推荐用于测试）**

```bash
# 备份数据
cp $RUNTIME_DATA ${RUNTIME_DATA}.backup

# 修改 boot_id 为假值
sudo sed -i 's/^boot_id=.*/boot_id=00000000-0000-0000-0000-000000000000/' $RUNTIME_DATA

# 重启程序
sudo systemctl restart komari-monitor
```

#### 3. 观察日志

```bash
# 实时查看日志
sudo journalctl -u komari-monitor -f
```

**期望看到的日志**：
```
System reboot detected, merging traffic data
```

#### 4. 验证数据变化

```bash
# 查看更新后的数据
cat $RUNTIME_DATA
```

**期望结果**：
- `boot_id`: 更新为新的系统 boot_id
- `boot_source_tx/rx`: 重置为当前系统流量（新的基准）
- `current_boot_tx/rx`: 重置为 0（新启动开始）
- `accumulated_tx/rx`: **增加了之前的 current_boot_tx/rx**

**计算验证**：
```
新的 accumulated_tx = 旧的 accumulated_tx + 旧的 current_boot_tx
新的 accumulated_rx = 旧的 accumulated_rx + 旧的 current_boot_rx

例如：
旧: accumulated_tx=500000000, current_boot_tx=100000000
新: accumulated_tx=600000000, current_boot_tx=0
```

#### 5. 恢复测试数据（如果使用了方式 B）

```bash
# 恢复备份
cp ${RUNTIME_DATA}.backup $RUNTIME_DATA
sudo systemctl restart komari-monitor
```

---

## 测试 2: 月度重置场景

**目的**: 验证程序在月度重置日能正确重置流量计数。

### 测试步骤

#### 1. 记录初始状态

```bash
# 查看当前月份和配置的重置日期
date +%m
grep reset_day /etc/komari-agent.conf  # 或 ~/.config/komari-agent.conf

# 查看当前运行时数据
cat $RUNTIME_DATA
```

记录 `last_reset_month` 的值。

#### 2. 模拟月度切换

**方式 A: 等待真实的月度切换（最真实但耗时）**

如果今天是每月的 `reset_day - 1` 日，可以等到第二天自动触发。

**方式 B: 修改 last_reset_month 模拟月度切换（推荐）**

```bash
# 备份数据
cp $RUNTIME_DATA ${RUNTIME_DATA}.backup

# 获取当前月份
CURRENT_MONTH=$(date +%-m)  # 移除前导零

# 计算上个月
PREVIOUS_MONTH=$((CURRENT_MONTH - 1))
if [ $PREVIOUS_MONTH -eq 0 ]; then
    PREVIOUS_MONTH=12
fi

echo "Current month: $CURRENT_MONTH"
echo "Setting last_reset_month to: $PREVIOUS_MONTH"

# 修改 last_reset_month
sudo sed -i "s/^last_reset_month=.*/last_reset_month=$PREVIOUS_MONTH/" $RUNTIME_DATA

# 重启程序
sudo systemctl restart komari-monitor
```

#### 3. 观察日志

```bash
sudo journalctl -u komari-monitor -f
```

**期望看到的日志**：
```
Monthly traffic reset triggered (configured day: 1, effective day: 1, current month: 1)
Traffic statistics reset completed
```

注意：
- `configured day`: 配置文件中的 `reset_day`
- `effective day`: 实际使用的重置日期（考虑月末情况）
- `current month`: 当前月份

#### 4. 验证数据变化

```bash
cat $RUNTIME_DATA
```

**期望结果**：
- `boot_source_tx/rx`: 更新为当前系统流量（新周期的基准）
- `current_boot_tx/rx`: 重置为 0
- `accumulated_tx/rx`: 重置为 0（**关键**）
- `last_reset_month`: 更新为当前月份

#### 5. 恢复测试数据

```bash
cp ${RUNTIME_DATA}.backup $RUNTIME_DATA
sudo systemctl restart komari-monitor
```

---

## 测试 3: 组合场景（重启 + 月度重置）

**目的**: 验证同时发生系统重启和月度切换时的处理逻辑。

### 测试步骤

#### 1. 备份数据

```bash
cp $RUNTIME_DATA ${RUNTIME_DATA}.backup
```

#### 2. 同时修改 boot_id 和 last_reset_month

```bash
CURRENT_MONTH=$(date +%-m)
PREVIOUS_MONTH=$((CURRENT_MONTH - 1))
if [ $PREVIOUS_MONTH -eq 0 ]; then
    PREVIOUS_MONTH=12
fi

sudo sed -i 's/^boot_id=.*/boot_id=00000000-0000-0000-0000-000000000000/' $RUNTIME_DATA
sudo sed -i "s/^last_reset_month=.*/last_reset_month=$PREVIOUS_MONTH/" $RUNTIME_DATA

sudo systemctl restart komari-monitor
```

#### 3. 观察日志

```bash
sudo journalctl -u komari-monitor -f
```

**期望看到的日志**：
```
System reboot detected, merging traffic data
Monthly traffic reset triggered (...)
Traffic statistics reset completed
```

#### 4. 验证数据变化

**期望结果**：月度重置优先级更高，最终所有流量计数应该被重置为 0。

```bash
cat $RUNTIME_DATA
```

检查：
- `accumulated_tx/rx`: 应该为 0（月度重置生效）
- `current_boot_tx/rx`: 应该为 0
- `last_reset_month`: 更新为当前月份

#### 5. 恢复数据

```bash
cp ${RUNTIME_DATA}.backup $RUNTIME_DATA
sudo systemctl restart komari-monitor
```

---

## 测试 4: 月末边界情况

**目的**: 验证 `reset_day` 超过当月天数时的智能处理。

### 测试场景

| reset_day | 月份 | 期望的 effective_reset_day |
|-----------|------|---------------------------|
| 31 | 1月 | 31 |
| 31 | 2月 (平年) | 28 |
| 31 | 2月 (闰年) | 29 |
| 31 | 4月 | 30 |
| 15 | 任意月 | 15 |

### 测试步骤

#### 1. 设置 reset_day=31

```bash
# 修改配置文件
sudo sed -i 's/^reset_day=.*/reset_day=31/' /etc/komari-agent.conf

# 重启程序
sudo systemctl restart komari-monitor
```

#### 2. 触发月度重置

按照 **测试 2** 的步骤触发月度重置。

#### 3. 检查日志

```bash
sudo journalctl -u komari-monitor | grep "Monthly traffic reset"
```

**期望看到**：
- 如果是 2 月：`configured day: 31, effective day: 28` (或 29)
- 如果是 4 月：`configured day: 31, effective day: 30`
- 如果是 1 月：`configured day: 31, effective day: 31`

---

## 测试 5: 流量累加验证

**目的**: 验证流量数据在正常运行时是否正确累加。

### 测试步骤

#### 1. 记录初始状态

```bash
cat $RUNTIME_DATA | grep -E "(boot_source|current_boot|accumulated)"
```

记录：
- `boot_source_tx/rx`
- `current_boot_tx/rx`
- `accumulated_tx/rx`

#### 2. 产生网络流量

```bash
# 下载一个大文件
wget http://speedtest.ftp.otenet.gr/files/test100Mb.db -O /tmp/test.db

# 或者使用 iperf3、curl 等工具产生流量
```

#### 3. 等待数据保存

等待至少 `network_interval * 10` 秒（默认是 100 秒）以确保数据已保存到磁盘。

#### 4. 再次查看数据

```bash
cat $RUNTIME_DATA | grep -E "(boot_source|current_boot|accumulated)"
```

**期望变化**：
- `current_boot_tx/rx`: **应该增加**（累加了新产生的流量）
- `boot_source_tx/rx`: 保持不变
- `accumulated_tx/rx`: 保持不变（只在重启或月度重置时改变）

#### 5. 计算总流量

```
总流量 TX = accumulated_tx + current_boot_tx + calibration_tx
总流量 RX = accumulated_rx + current_boot_rx + calibration_rx
```

可以通过程序 API 或日志查看显示给用户的总流量，验证计算是否正确。

---

## 测试 6: 校准值测试

**目的**: 验证 `calibration_tx/rx` 参数能正确调整显示的总流量。

### 测试步骤

#### 1. 记录当前显示的总流量

通过 Web 界面或 API 查看当前显示的流量值。

#### 2. 设置校准值

```bash
# 假设你想增加 1GB (1073741824 字节) 的上传流量
sudo sed -i 's/^calibration_tx=.*/calibration_tx=1073741824/' /etc/komari-agent.conf

# 重启程序
sudo systemctl restart komari-monitor
```

#### 3. 验证新的总流量

新的显示值应该 = 旧值 + 1073741824 (1GB)

---

## 常见问题排查

### 1. 日志中没有看到 "System reboot detected"

**可能原因**：
- 不在 Linux 系统上
- `/proc/sys/kernel/random/boot_id` 文件不存在
- `boot_id` 修改不正确

**解决方法**：
```bash
# 检查 boot_id 文件是否存在
cat /proc/sys/kernel/random/boot_id

# 确保运行时数据中的 boot_id 与系统不同
cat $RUNTIME_DATA | grep boot_id
```

### 2. 月度重置没有触发

**可能原因**：
- `last_reset_month` 设置不正确
- 当前日期小于 `reset_day`

**检查条件**：
```bash
# 月度重置的触发条件（来自代码）
# should_reset_traffic() 返回 true 需要满足：
# 1. current_month != last_reset_month
# 2. months_diff > 1 OR (months_diff == 1 AND current_day >= effective_reset_day)

# 检查当前日期
date +"%Y-%m-%d (day: %d, month: %m)"

# 检查配置
grep reset_day /etc/komari-agent.conf
```

### 3. 流量数据没有保存

**可能原因**：
- 时间间隔太短（默认每 10 个 `network_interval` 保存一次）
- 文件权限问题

**解决方法**：
```bash
# 检查文件权限
ls -la $RUNTIME_DATA

# 检查日志错误
sudo journalctl -u komari-monitor | grep -i error
```

---

## 数据流转图

理解流量数据在不同状态下的流转：

```
初始安装:
  accumulated = 0
  current_boot = 0
  boot_source = 当前系统流量

正常运行:
  current_boot = 当前系统流量 - boot_source  (持续增长)
  accumulated 不变
  显示总流量 = accumulated + current_boot + calibration

系统重启:
  accumulated += current_boot  (合并上次启动的流量)
  current_boot = 0  (重置)
  boot_source = 当前系统流量  (新基准)

月度重置:
  accumulated = 0  (重置)
  current_boot = 0  (重置)
  boot_source = 当前系统流量  (新基准)
  last_reset_month = 当前月份
```

---

## 自动化测试建议

如果你需要在 CI/CD 中集成测试，可以编写自动化脚本：

```bash
#!/bin/bash
# automated_test.sh

set -e

# 1. 安装程序
# 2. 初始化配置
# 3. 运行各项测试
./tests/traffic_test.sh reboot
# 验证结果
# 恢复数据

./tests/traffic_test.sh monthly
# 验证结果
# 恢复数据

# 4. 清理环境
```

---

## 总结

通过以上测试，你可以全面验证：

- ✅ 系统重启后流量数据正确合并
- ✅ 月度重置按计划执行
- ✅ 月末边界情况智能处理
- ✅ 流量数据正确累加
- ✅ 校准值正确应用
- ✅ 组合场景正确处理

建议在生产环境部署前，至少完成 **测试 1、2、5** 以确保核心功能正常。

如果发现任何问题，请查看日志文件并参考常见问题排查部分。
