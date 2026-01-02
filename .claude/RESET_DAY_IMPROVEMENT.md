# reset_day 范围扩展改进说明

## 改进概述

将 `reset_day` 参数范围从 **1-28** 扩展到 **1-31**，并实现了智能的月末日期处理逻辑。

---

## 改进动机

### 旧方案的限制

- `reset_day` 限制为 1-28，无法设置月末重置
- 用户如果想在月末重置流量，只能使用 28 号
- 对于 VPS 账单周期通常是按月计算的用户不够灵活

### 新方案的优势

- 支持 1-31 全范围日期设置
- 自动适配不同月份的天数
- 更符合实际使用场景（如账单周期）

---

## 技术实现

### 1. 新增函数

#### `get_days_in_current_month() -> u8`

**功能**: 获取当前月份的天数（28-31）

**实现逻辑**:
- 识别大月（31 天）：1/3/5/7/8/10/12 月
- 识别小月（30 天）：4/6/9/11 月
- 识别二月：平年 28 天，闰年 29 天
- 闰年判断：`(year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)`

**代码位置**: [network_saver.rs:37-63](../src/get_info/network/network_saver.rs#L37-L63)

```rust
fn get_days_in_current_month() -> u8 {
    let now = match OffsetDateTime::now_local() {
        Ok(now) => now,
        Err(_) => OffsetDateTime::now_utc(),
    };

    let year = now.year();
    let month = now.month();

    match month {
        time::Month::January | time::Month::March | time::Month::May
        | time::Month::July | time::Month::August | time::Month::October
        | time::Month::December => 31,
        time::Month::April | time::Month::June
        | time::Month::September | time::Month::November => 30,
        time::Month::February => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29  // 闰年
            } else {
                28  // 平年
            }
        }
    }
}
```

#### `get_effective_reset_day(reset_day: u8) -> u8`

**功能**: 获取实际生效的重置日期

**实现逻辑**:
- 如果 `reset_day` ≤ 当月天数，直接使用
- 如果 `reset_day` > 当月天数，使用当月最后一天

**代码位置**: [network_saver.rs:65-70](../src/get_info/network/network_saver.rs#L65-L70)

```rust
fn get_effective_reset_day(reset_day: u8) -> u8 {
    let days_in_month = get_days_in_current_month();
    reset_day.min(days_in_month)
}
```

### 2. 修改的函数

#### `should_reset_traffic(last_reset_month: u8, reset_day: u8) -> bool`

**旧逻辑**:
```rust
current_month != last_reset_month && current_day >= reset_day
```

**新逻辑**:
```rust
let effective_reset_day = get_effective_reset_day(reset_day);
current_month != last_reset_month && current_day >= effective_reset_day
```

**改进点**:
- 使用有效重置日期而非直接使用配置值
- 自动适配月末情况

### 3. 日志输出改进

**旧日志**:
```
Monthly traffic reset triggered (reset day: 1, current month: 2)
```

**新日志**:
```
Monthly traffic reset triggered (configured day: 31, effective day: 28, current month: 2)
```

**改进点**:
- 同时显示配置值和实际生效值
- 让用户清楚了解重置行为

---

## 使用场景示例

### 场景 1: 每月 1 号重置（默认）

```bash
./komari-monitor-rs --reset-day 1
```

**行为**:
- 所有月份都在 1 号重置
- 固定行为，无月末适配

### 场景 2: 每月 15 号重置

```bash
./komari-monitor-rs --reset-day 15
```

**行为**:
- 所有月份都在 15 号重置
- 固定行为，无月末适配

### 场景 3: 每月最后一天重置 ⭐

```bash
./komari-monitor-rs --reset-day 31
```

**行为**:
| 月份 | 天数 | 实际重置日期 |
|------|------|--------------|
| 1 月 | 31 | 31 号 |
| 2 月（平年）| 28 | **28 号** |
| 2 月（闰年）| 29 | **29 号** |
| 3 月 | 31 | 31 号 |
| 4 月 | 30 | **30 号** |
| 5 月 | 31 | 31 号 |
| 6 月 | 30 | **30 号** |
| 7 月 | 31 | 31 号 |
| 8 月 | 31 | 31 号 |
| 9 月 | 30 | **30 号** |
| 10 月 | 31 | 31 号 |
| 11 月 | 30 | **30 号** |
| 12 月 | 31 | 31 号 |

### 场景 4: 设置 30 号重置

```bash
./komari-monitor-rs --reset-day 30
```

**行为**:
| 月份 | 天数 | 实际重置日期 |
|------|------|--------------|
| 1 月 | 31 | 30 号 |
| 2 月（平年）| 28 | **28 号** |
| 2 月（闰年）| 29 | **29 号** |
| 其他月 | 30/31 | 30 号 |

---

## 测试用例

### 单元测试场景

#### 1. 测试闰年判断
```rust
// 2024 年是闰年
assert_eq!(is_leap_year(2024), true);
// 2100 年不是闰年（能被 100 整除但不能被 400 整除）
assert_eq!(is_leap_year(2100), false);
// 2000 年是闰年（能被 400 整除）
assert_eq!(is_leap_year(2000), true);
```

#### 2. 测试月份天数
```rust
// 假设当前是 2024 年 2 月（闰年）
assert_eq!(get_days_in_current_month(), 29);

// 假设当前是 2025 年 2 月（平年）
assert_eq!(get_days_in_current_month(), 28);

// 假设当前是 2025 年 4 月
assert_eq!(get_days_in_current_month(), 30);

// 假设当前是 2025 年 1 月
assert_eq!(get_days_in_current_month(), 31);
```

#### 3. 测试有效重置日期
```rust
// 当前 2 月平年（28 天）
assert_eq!(get_effective_reset_day(31), 28);  // 超出范围，使用月末
assert_eq!(get_effective_reset_day(28), 28);  // 正好是月末
assert_eq!(get_effective_reset_day(15), 15);  // 在范围内

// 当前 4 月（30 天）
assert_eq!(get_effective_reset_day(31), 30);  // 超出范围，使用月末
assert_eq!(get_effective_reset_day(30), 30);  // 正好是月末

// 当前 1 月（31 天）
assert_eq!(get_effective_reset_day(31), 31);  // 正好是月末
assert_eq!(get_effective_reset_day(35), 31);  // 超出范围，使用月末
```

### 集成测试场景

#### 1. 跨月重置测试
```
1. 设置 reset_day=31
2. 模拟 1 月 31 日：应该触发重置
3. 模拟 2 月 28 日（平年）：应该触发重置
4. 模拟 2 月 29 日（闰年）：应该触发重置
5. 模拟 4 月 30 日：应该触发重置
```

#### 2. 日志输出测试
```
设置 reset_day=31
当前 2 月 28 日（平年）
期望日志：
"Monthly traffic reset triggered (configured day: 31, effective day: 28, current month: 2)"
```

---

## 兼容性说明

### 向前兼容

✅ **完全兼容** - 旧的配置文件无需修改

- 旧配置 `reset_day=1` 到 `reset_day=28` 的行为完全不变
- 新支持的范围 `reset_day=29` 到 `reset_day=31` 是新增功能
- 无破坏性变更

### 配置文件

旧配置文件依然有效：
```ini
reset_day=1   # 依然有效
reset_day=15  # 依然有效
reset_day=28  # 依然有效
```

新增支持：
```ini
reset_day=29  # 新增支持
reset_day=30  # 新增支持
reset_day=31  # 新增支持
```

---

## 文档更新清单

### 更新的文件

1. ✅ [src/command_parser.rs](../src/command_parser.rs)
   - 参数注释：1-28 → 1-31
   - clamp 范围：clamp(1, 28) → clamp(1, 31)

2. ✅ [src/get_info/network/network_saver.rs](../src/get_info/network/network_saver.rs)
   - 新增 `get_days_in_current_month()`
   - 新增 `get_effective_reset_day()`
   - 修改 `should_reset_traffic()`
   - 改进日志输出

3. ✅ [NETWORK_RESET_GUIDE.md](../NETWORK_RESET_GUIDE.md)
   - 更新范围说明
   - 添加月末适配说明
   - 添加示例时间线
   - 更新字段说明表

4. ✅ [komari-network.conf.example](../komari-network.conf.example)
   - 更新注释说明
   - 添加月末重置示例

5. ✅ [CHANGELOG_NETWORK.md](../CHANGELOG_NETWORK.md)
   - 添加月末智能处理说明
   - 添加示例

6. ✅ [.claude/REFACTORING_SUMMARY.md](../.claude/REFACTORING_SUMMARY.md)
   - 更新参数范围
   - 添加新增函数说明
   - 更新已知限制

---

## 性能影响

### 新增计算开销

**每次重置检查增加的开销**:
1. `get_days_in_current_month()`: 1 次时间获取 + 1 次模式匹配
2. `get_effective_reset_day()`: 1 次 `min()` 比较

**评估**:
- 单次开销 < 1 微秒
- 每 10 秒检查一次（默认 `network_interval`）
- 对性能影响可忽略不计

### 内存影响

- 无新增持久化字段
- 仅增加临时变量计算
- 内存影响：0 bytes

---

## 边界情况处理

### 1. 非法值处理

```rust
// command_parser.rs 中已有 clamp 保护
reset_day: self.reset_day.clamp(1, 31)
```

输入范围：
- `reset_day = 0` → 自动修正为 1
- `reset_day = 255` → 自动修正为 31

### 2. 时间获取失败

```rust
let now = match OffsetDateTime::now_local() {
    Ok(now) => now,
    Err(_) => OffsetDateTime::now_utc(),  // 回退到 UTC
};
```

### 3. 跨时区场景

- 优先使用本地时间（`now_local()`）
- 失败时回退到 UTC（`now_utc()`）
- 可能在跨时区服务器上有 1 天偏差
- 建议：在配置文件中明确时区设置（未来改进）

---

## 总结

### 改进亮点

✅ **用户体验**：支持月末重置，更灵活
✅ **智能适配**：自动处理不同月份天数
✅ **向前兼容**：旧配置无需修改
✅ **性能友好**：开销可忽略不计
✅ **代码清晰**：函数职责明确，易于维护

### 适用场景

- VPS 账单周期通常从月末开始的用户
- 需要在每月最后一天统计流量的场景
- 避免手动调整不同月份天数的麻烦

### 后续改进建议

1. **时区配置**: 支持用户指定时区（当前依赖系统时区）
2. **自定义周期**: 支持按周、按季度重置
3. **预警功能**: 在重置前 X 天提醒用户

---

**日期**: 2026-01-02
**作者**: Claude Code Agent
**版本**: v0.4.0
