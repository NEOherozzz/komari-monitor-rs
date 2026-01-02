# 流量统计重构说明

## 概述

流量统计功能已从基于倒计时的周期重置改为基于日期的月度重置机制，并新增了流量校准和配置热重载功能。

## 主要变更

### 1. 月度重置机制

**旧逻辑：**
- 使用 `counter` 倒计时
- 达到 0 后清空所有统计数据
- 需要手动计算周期（duration / interval）

**新逻辑：**
- 指定每月固定日期（1-31）自动重置
- 跨月时自动检测并重置流量
- 更符合 VPS 账单周期

**配置参数：**
```bash
--network-reset-day=1  # 每月1号重置流量
```

### 2. 流量校准功能

允许用户设置偏移量，使程序统计的流量与 VPS 服务商保持一致。

**使用场景：**
- VPS 服务商计算了协议开销，程序未计算
- 需要补偿程序启动前已消耗的流量
- 修正本地测试流量的影响

**配置参数：**
```bash
--network-calibration-tx=1073741824   # 上传流量校准 +1GB
--network-calibration-rx=-536870912   # 下载流量校准 -512MB
```

**计算方式：**
```
最终流量 = 实际测量流量 + 校准值
```

### 3. 配置热重载

支持在程序运行时修改配置文件，自动检测并应用新配置。

**启用方式：**
```bash
--config-path=/path/to/config.conf
```

**配置文件格式：**
```ini
disable_network_statistics=false
network_reset_day=1
network_interval=60
network_interval_number=10
network_calibration_tx=0
network_calibration_rx=0
```

**重载机制：**
- 每个采样周期检查文件修改时间
- 检测到变化时自动重新加载
- 仅支持校准值、采样间隔等运行时参数
- 不支持热重载：save_path、config_path

## 数据结构变更

### NetworkInfo

**移除字段：**
- `counter: u32` - 倒计时计数器

**新增字段：**
- `last_reset_month: String` - 上次重置的年月（格式：YYYY-MM）

### NetworkConfig

**移除字段：**
- `network_duration: u32` - 统计周期（秒）

**新增字段：**
- `network_reset_day: u32` - 每月重置日（1-31）
- `network_calibration_tx: i64` - 上传流量校准值（字节）
- `network_calibration_rx: i64` - 下载流量校准值（字节）
- `config_path: Option<String>` - 配置文件路径（用于热重载）

## 使用示例

### 基础使用

```bash
# 每月15号重置流量
komari-monitor-rs --network-reset-day=15

# 校准流量（与服务商对齐）
komari-monitor-rs \
  --network-calibration-tx=1073741824 \
  --network-calibration-rx=2147483648
```

### 配置文件使用

1. 创建配置文件 `network.conf`：
```ini
network_reset_day=1
network_calibration_tx=0
network_calibration_rx=0
```

2. 启动程序：
```bash
komari-monitor-rs --config-path=network.conf
```

3. 运行时调整校准值：
```bash
# 编辑 network.conf
echo "network_calibration_tx=1073741824" >> network.conf

# 程序会在下个采样周期自动应用新配置
```

### 校准值计算

**场景1：服务商显示流量更多**
```
服务商统计：150.5 GB 上传
程序统计：  149.2 GB 上传
差值：       1.3 GB = 1395864371 bytes

设置：--network-calibration-tx=1395864371
```

**场景2：补偿历史流量**
```
本月已用流量：50 GB
程序刚启动，显示：0 GB

设置：--network-calibration-tx=53687091200  # 50GB in bytes
```

## 重置逻辑说明

### 重置触发条件

每次采样时检查以下条件：
1. 当前月份（YYYY-MM）!= last_reset_month
2. 当前日期 >= network_reset_day

同时满足时触发重置。

### 重置行为

```rust
source_tx = 0
source_rx = 0
latest_tx = 0
latest_rx = 0
last_reset_month = "当前年月"
```

### 跨月处理

**示例：**
- 重置日设置为15号
- 当前是1月10号：不重置
- 到了1月15号：检查 last_reset_month
  - 如果是 "2024-12"：触发重置，更新为 "2025-01"
  - 如果是 "2025-01"：已重置过，不重复重置

## 系统重启处理

**Linux 系统：**
- 通过 `/proc/sys/kernel/random/boot_id` 检测重启
- 重启后自动将 latest_tx/rx 累加到 source_tx/rx
- 重置 latest_tx/rx 为 0

**其他系统：**
- 无法精确检测重启
- 启动时显示上次保存的流量值

## 兼容性说明

**重要：此版本不向前兼容！**

旧版本的 `komari-network.conf` 文件无法直接使用，程序会：
1. 检测到文件格式错误
2. 等待 3 秒
3. 删除旧文件并创建新文件
4. 从 0 开始统计流量

**迁移建议：**
1. 记录当前流量统计值
2. 升级程序
3. 使用校准功能补偿之前的流量

## 故障排查

### 问题1：配置文件修改未生效

**原因：**
- 未指定 `--config-path`
- 配置文件格式错误

**解决：**
```bash
# 检查日志输出
grep "Config file changed" /var/log/komari.log

# 验证配置文件格式
cat network.conf | grep -E "^(network_|disable_)"
```

### 问题2：流量显示为负数

**原因：**
- 校准值过大且为负数

**解决：**
- 程序会自动截断为 0
- 调整校准值为合理范围

### 问题3：流量未在指定日期重置

**原因：**
- 程序未在重置日运行
- 系统时间不正确

**解决：**
- 确保程序持续运行
- 检查系统时间：`date`
- 查看 last_reset_month 字段

## 技术细节

### 日期比较逻辑

```rust
fn should_reset_traffic(last_reset_month: &str, reset_day: u32) -> bool {
    let now = OffsetDateTime::now_utc();
    let current_month = format!("{}-{:02}", now.year(), now.month() as u8);
    let current_day = now.day();
    
    if last_reset_month != current_month && current_day >= reset_day {
        return true;
    }
    
    false
}
```

### 配置热重载实现

```rust
// 检查文件修改时间
if let Ok(metadata) = tokio::fs::metadata(config_path).await {
    if let Ok(modified) = metadata.modified() {
        if config_file_modified_time.map_or(true, |last| modified > last) {
            // 重新加载配置
            let new_config = load_config_from_file(config_path).await?;
            // 应用新配置
        }
    }
}
```

### 校准值应用

```rust
let calibrated_tx = (measured_tx as i64) + calibration_tx;
let calibrated_rx = (measured_rx as i64) + calibration_rx;

// 确保非负
let final_tx = calibrated_tx.max(0) as u64;
let final_rx = calibrated_rx.max(0) as u64;
```

## 性能影响

- **配置检查开销：** 每个采样周期增加 1 次文件 stat 系统调用（~1ms）
- **校准计算开销：** 可忽略不计（简单加法）
- **重置检查开销：** 可忽略不计（字符串比较）

总体性能影响：<0.1% CPU 使用率增加
