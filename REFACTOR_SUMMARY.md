# 流量统计重构完成总结

## 已完成的修改

### 1. 配置结构更新 (command_parser.rs)

**移除参数：**
- `network_duration` - 统计周期（秒）

**新增参数：**
- `network_reset_day: u32` - 每月重置日（默认：1号）
- `network_calibration_tx: i64` - 上传流量校准值（默认：0）
- `network_calibration_rx: i64` - 下载流量校准值（默认：0）
- `config_path: Option<String>` - 配置文件路径（用于热重载）

### 2. 数据结构重构 (network_saver.rs)

**NetworkInfo 结构体变更：**
- 移除：`counter: u32`
- 新增：`last_reset_month: String`（格式：YYYY-MM）

### 3. 核心功能实现

#### A. 月度重置机制
```rust
fn should_reset_traffic(last_reset_month: &str, reset_day: u32) -> bool
```
- 自动检测月份变化
- 在指定日期重置流量统计
- 避免重复重置

#### B. 流量校准功能
```rust
let calibrated_tx = (measured_tx as i64) + calibration_tx;
let calibrated_rx = (measured_rx as i64) + calibration_rx;
let final_tx = calibrated_tx.max(0) as u64;
let final_rx = calibrated_rx.max(0) as u64;
```
- 支持正负偏移量
- 自动防止负数溢出
- 实时应用到发送的流量数据

#### C. 配置热重载
```rust
async fn load_config_from_file(config_path: &str) -> Result<NetworkConfig, String>
```
- 监控配置文件修改时间
- 自动重新加载配置
- 应用到当前运行实例

### 4. 编码解码逻辑更新

**encode() 方法更新：**
- 添加新字段序列化：reset_day, calibration_tx/rx, config_path
- 移除：network_duration, counter

**decode() 方法更新：**
- 解析新字段
- 兼容 config_path 可选字段
- 保留向后兼容性（可选字段）

### 5. 初始化逻辑重构

**get_or_init_latest_network_info() 改进：**
- 使用 `time::OffsetDateTime` 获取当前时间
- 初始化时设置 `last_reset_month`
- 简化 boot_id 检测逻辑
- 优化配置变更处理

### 6. 主循环重写

**network_saver() 函数增强：**
- 添加配置文件监控循环
- 集成重置日期检查
- 实现校准值应用
- 保持原有的批量写入优化

## 关键改进

### 性能优化
- 批量写入机制保持不变
- 仅增加轻量级文件 stat 调用
- 校准计算开销可忽略

### 用户体验
- 更符合 VPS 账单周期
- 无需计算 duration 参数
- 支持运行时调整校准值
- 配置热重载无需重启

### 可维护性
- 移除复杂的 counter 倒计时逻辑
- 日期检查更直观
- 代码结构更清晰

## 使用示例

### 基础配置
```bash
komari-monitor-rs \
  --network-reset-day=1 \
  --network-calibration-tx=0 \
  --network-calibration-rx=0
```

### 配置文件热重载
```bash
# 创建配置文件
cat > network.conf <<EOF
network_reset_day=1
network_calibration_tx=1073741824
network_calibration_rx=2147483648
EOF

# 启动程序
komari-monitor-rs --config-path=network.conf

# 运行时修改（程序会自动检测）
echo "network_calibration_tx=2147483648" >> network.conf
```

## 测试建议

### 1. 基础功能测试
```bash
# 测试月度重置
# 1. 修改系统时间到月末
# 2. 等待跨月
# 3. 验证流量重置

# 测试校准功能
# 1. 设置校准值
# 2. 检查报告的流量是否正确应用偏移

# 测试热重载
# 1. 启动程序
# 2. 修改配置文件
# 3. 检查日志确认重载成功
```

### 2. 边界条件测试
- 校准值为负数且绝对值大于实际流量
- reset_day 设置为 31 号在 2 月
- 配置文件格式错误
- 配置文件被删除

### 3. 兼容性测试
- 从旧版本升级（会清空旧数据）
- 系统重启后的流量累积
- Linux 和 Windows 平台差异

## 已知限制

1. **不向前兼容**：旧版本的配置文件无法使用
2. **日期检测**：依赖系统时间，时间调整可能影响重置
3. **配置热重载**：仅支持部分参数（calibration, interval 等）
4. **月份特殊处理**：reset_day=31 在 2 月会在 2 月 28/29 日重置

## 后续改进建议

1. **时区支持**：允许用户指定时区而非使用 UTC
2. **重置通知**：重置时发送通知到监控系统
3. **历史记录**：保存每月流量历史
4. **Web 界面**：可视化配置校准值
5. **自动校准**：定期与服务商 API 同步自动调整

## 文件清单

**修改的文件：**
- `src/command_parser.rs` - 配置参数定义
- `src/get_info/network/network_saver.rs` - 核心逻辑实现

**新增的文件：**
- `network-config.example.conf` - 配置文件示例
- `NETWORK_REFACTOR.md` - 详细功能说明
- `REFACTOR_SUMMARY.md` - 本文件

**需要测试的文件：**
- `src/data_struct.rs` - 确认数据结构集成
- `src/main.rs` - 确认参数传递正确

## 编译验证

请运行以下命令验证编译：
```bash
cargo check
cargo build --release
```

如有编译错误，请检查：
1. `time` crate 版本兼容性
2. 所有字段访问是否更新
3. 类型转换是否正确
