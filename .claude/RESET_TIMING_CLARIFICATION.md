# reset_day 和流量重置时间关系说明文档

## 文档目的

本文档旨在明确说明 `reset_day` 配置参数与流量实际重置时间的关系，解决用户对"何时重置"的疑问。

---

## 核心问题

**问题**：`reset_day=5` 是在 5 号的什么时候重置流量？是 5 号开始时？5 号结束时？还是其他时间？

**答案**：流量重置发生在 `reset_day` **当天**（从 00:00:00 到 23:59:59 之间），具体时刻取决于程序的检查周期。

---

## 技术实现

### 重置触发条件

程序每隔 `network_interval` 秒（默认 10 秒）检查一次，当满足以下**两个条件**时触发重置：

```rust
// 条件 1：月份已经改变
current_month != last_reset_month

// 条件 2：到达或超过重置日期
current_day >= effective_reset_day
```

### 关键代码逻辑

```rust
fn should_reset_traffic(last_reset_month: u8, reset_day: u8) -> bool {
    let current_month = get_current_month();
    let current_day = get_current_day();
    let effective_reset_day = get_effective_reset_day(reset_day);

    // 同月不重置
    if current_month == last_reset_month {
        return false;
    }

    // 计算月份差
    let months_diff = if current_month > last_reset_month {
        current_month - last_reset_month
    } else {
        12 - last_reset_month + current_month  // 跨年
    };

    // 重置条件：
    // 1. 超过1个月（处理长时间停机）
    // 2. 恰好1个月且到达重置日
    months_diff > 1 || current_day >= effective_reset_day
}
```

**关键点**：
- `current_day >= effective_reset_day` 使用 `>=` 运算符
- 这意味着在 reset_day 当天（从 00:00 开始）就满足条件
- 配合 `current_month != last_reset_month` 确保每月只重置一次

---

## 时间线示例

### 示例 1：reset_day=5（每月 5 号重置）

```
1月4日 23:59:59  | current_month=1, last_reset_month=1, current_day=4
                 | 4 < 5 → 不重置
                 | （同月且未到重置日）

1月5日 00:00:00  | current_month=1, last_reset_month=1, current_day=5
                 | 5 >= 5 但 current_month == last_reset_month
                 | → 不重置（同月）
                 | （如果是首次运行，会设置 last_reset_month=1）

2月4日 23:59:59  | current_month=2, last_reset_month=1, current_day=4
                 | 月份改变 但 4 < 5
                 | → 不重置（未到重置日）

2月5日 00:00:10  | current_month=2, last_reset_month=1, current_day=5
(首次检查)       | 月份改变 且 5 >= 5
                 | → ✅ 触发重置！
                 | 设置 last_reset_month=2

2月5日 00:00:20  | current_month=2, last_reset_month=2, current_day=5
(第二次检查)     | current_month == last_reset_month
                 | → 不重置（本月已重置）

2月6日           | current_month=2, last_reset_month=2, current_day=6
                 | current_month == last_reset_month
                 | → 不重置（本月已重置）
```

### 示例 2：reset_day=31（每月最后一天重置）

```
1月30日 23:59:59 | current_day=30, effective_reset_day=31
                | 30 < 31 → 不重置

1月31日 00:00:10 | current_day=31, effective_reset_day=31
                | 31 >= 31 且月份改变
                | → ✅ 触发重置！

2月27日 23:59:59 | current_day=27, effective_reset_day=28（2月平年）
                | 27 < 28 → 不重置

2月28日 00:00:10 | current_day=28, effective_reset_day=28
                | 28 >= 28 且月份改变
                | → ✅ 触发重置！
                | （2月只有28天，自动使用最后一天）

3月30日 23:59:59 | current_day=30, effective_reset_day=31
                | 30 < 31 → 不重置

3月31日 00:00:10 | current_day=31, effective_reset_day=31
                | 31 >= 31 且月份改变
                | → ✅ 触发重置！
```

---

## 重要概念澄清

### 1. 重置日 = 流量归零日

- `reset_day=1`：每月 **1 号当天** 重置，不是 2 号，也不是月末
- `reset_day=15`：每月 **15 号当天** 重置
- `reset_day=31`：每月 **最后一天当天** 重置

### 2. 重置不会提前或延后

- ✅ **正确**：重置发生在 reset_day 当天（00:00 - 23:59）
- ❌ **错误**：重置在 reset_day 前一天 23:59
- ❌ **错误**：重置在 reset_day 后一天 00:00

### 3. 精确时刻由检查周期决定

- 程序每 `network_interval` 秒检查一次（默认 10 秒）
- 如果程序持续运行，重置通常在 00:00:00 - 00:00:10 之间
- 如果程序在重置日后启动，会立即触发重置

### 4. 月末智能处理

- `reset_day=31` 在 2 月会自动调整为 28 或 29 日
- 调整后的日期就是该月的"重置日"
- 重置依然在该日期当天发生

---

## 实际应用场景

### 场景 1：VPS 账单从每月 1 号开始

```bash
reset_day=1
```

**效果**：
- 1 月 1 日：重置（1月账单周期开始）
- 2 月 1 日：重置（2月账单周期开始）
- 流量统计与 VPS 账单周期完全同步

### 场景 2：VPS 账单从每月 15 号开始

```bash
reset_day=15
```

**效果**：
- 1 月 15 日：重置（本月账单周期开始）
- 2 月 15 日：重置（下月账单周期开始）
- 适合账单周期非标准月初的场景

## 用户常见疑问

### Q1：为什么我的流量在 5 号重置，而不是 4 号或 6 号？

**A**：因为你设置了 `reset_day=5`，程序严格在 5 号当天触发重置。

### Q2：能否在 5 号的 12:00 重置，而不是 00:00？

**A**：目前不支持指定具体时刻，只能指定日期。重置会在该日期当天的首次检查时触发（通常是 00:00 - 00:00:10）。

### Q3：我设置 reset_day=31，为什么 2 月 28 日就重置了？

**A**：这是月末智能处理功能。2 月只有 28 天（或闰年 29 天），程序自动将 31 调整为该月最后一天。这样你可以用 `reset_day=31` 实现"每月最后一天重置"的效果。

### Q4：如果程序在 2月10日 启动，reset_day=5，会发生什么？

**A**：程序会检测到 current_day(10) >= reset_day(5) 且 current_month != last_reset_month，会立即触发重置。这是为了确保在重置日期之后启动时，不会错过重置。

### Q5：如何查看上次重置时间？

**A**：查看运行时数据文件：
```bash
# Linux (root)
cat /var/lib/komari-monitor/network-data.conf

# Linux (非root)
cat ~/.local/share/komari-monitor/network-data.conf

# 查看 last_reset_month 字段
```

---

## 代码改进说明

本次改进主要是 **文档和注释的增强**，没有改变核心逻辑：

### 改进文件列表

1. **src/get_info/network/network_saver.rs**
   - 增强了 `calculate_initial_last_reset_month()` 的注释
   - 增强了 `should_reset_traffic()` 的注释
   - 增强了 `get_effective_reset_day()` 的注释
   - 添加详细的重置时间行为说明和示例

2. **NETWORK_RESET_GUIDE.md**
   - 添加"重置时间说明"部分
   - 详细说明重置发生在当天（00:00 - 23:59）
   - 增强"工作原理 > 重置机制"部分
   - 添加更详细的时间线示例

3. **komari-agent.conf.example**
   - 增强 `reset_day` 参数的注释
   - 添加重置时间行为说明
   - 添加更多使用示例

4. **.claude/RESET_DAY_IMPROVEMENT.md**
   - 新增"reset_day 和流量重置时间的关系"专门章节
   - 添加详细的触发逻辑说明
   - 添加实例分析表格
   - 增强使用场景示例，添加重置时间范围

5. **.claude/RESET_TIMING_CLARIFICATION.md**（本文件）
   - 专门说明 reset_day 与重置时间的关系
   - 提供详细的时间线示例
   - 解答常见疑问

---

## 总结

**核心要点**：
1. 流量重置发生在 `reset_day` **当天**（00:00 - 23:59）
2. 具体时刻由检查周期决定，通常在 00:00:00 - 00:00:10 之间
3. 不会提前或延后，严格按照配置的日期执行
4. 月末智能处理确保 reset_day=31 在所有月份都能正常工作
5. 延迟启动会立即触发重置（如果已过重置日期）

**日期**: 2026-01-05  
**作者**: Copilot Code Agent  
**版本**: v0.4.1
