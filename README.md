# Komari-Monitor-rs

![](https://hitscounter.dev/api/hit?url=https%3A%2F%2Fgithub.com%2FNEOherozzz%2Fkomari-monitor-rs&label=&icon=github&color=%23160d27)
![komari-monitor-rs](https://socialify.git.ci/NEOherozzz/komari-monitor-rs/image?custom_description=Komari+%E7%AC%AC%E4%B8%89%E6%96%B9+Agent+%7C+%E9%AB%98%E6%80%A7%E8%83%BD+%7C+Fork+Enhanced&description=1&font=KoHo&forks=1&issues=1&language=1&name=1&owner=1&pattern=Floating+Cogs&pulls=1&stargazers=1&theme=Auto)

## About

`Komari-Monitor-rs` 是一个适用于 [komari-monitor](https://github.com/komari-monitor) 监控服务的第三方**高性能**监控
Agent

致力于实现[原版 Agent](https://github.com/komari-monitor/komari-agent) 的所有功能，并拓展更多功能

## Fork 修改说明

本项目 Fork 自 [GenshinMinecraft/komari-monitor-rs](https://github.com/GenshinMinecraft/komari-monitor-rs)，并进行了以下重要修改和增强：

### 🚀 主要功能增强

#### 1. 网络流量统计重构
- **按月重置模式**：将流量重置从定时周期改为每月固定日期重置
  - 新增 `--reset-day` 参数（1-31），支持智能月末处理
  - 自动适配不同月份的天数（2 月 28/29 天，4/6/9/11 月 30 天等）
- **流量校准功能**：支持与 VPS 服务商流量对齐
  - `--calibration-tx`：上传流量校准值
  - `--calibration-rx`：下载流量校准值
- **配置热重载**：无需重启程序即可应用配置变更
- **移除废弃参数**：`--network-duration`、`--network-interval-number`

详细文档：[NETWORK_RESET_GUIDE.md](NETWORK_RESET_GUIDE.md) | [CHANGELOG_NETWORK.md](CHANGELOG_NETWORK.md)

#### 2. 安装和管理工具
- **kagent.sh 脚本**：新增一键安装和管理工具
  - 支持无交互安装模式
  - 重新安装保护（自动保留旧配置和网络数据）
  - 完整的配置文件生成和管理
  - 网络数据目录自动管理

#### 3. 配置文件模式
- 支持配置文件持久化存储
- 配置变更时保留流量数据（不再强制重置）
- 改进的配置文件格式，包含详细注释

### 🐛 修复和改进

- **跨平台编译修复**
  - 修复 Windows 编译错误（未使用导入、死代码警告）
  - 修复 macOS 编译时 libc 依赖缺失问题
  - 改进跨平台兼容性

- **运行时改进**
  - 修复 root 用户检测逻辑
  - 优化网络数据初始化流程
  - 改进系统重启检测（Linux 使用 boot_id，Windows 自动合并流量）

- **安装脚本改进**
  - 移除 `--ws-server` 参数及相关交互
  - 简化安装流程（仅需 HTTP 地址和 Token）
  - 修复安装时保留旧网络数据的问题

### 📚 文档增强

新增以下文档：
- [NETWORK_RESET_GUIDE.md](NETWORK_RESET_GUIDE.md) - 流量统计用户指南
- [CHANGELOG_NETWORK.md](CHANGELOG_NETWORK.md) - 网络功能变更日志
- [.claude/REFACTORING_SUMMARY.md](.claude/REFACTORING_SUMMARY.md) - 技术重构总结
- [.claude/RESET_DAY_IMPROVEMENT.md](.claude/RESET_DAY_IMPROVEMENT.md) - reset_day 功能扩展说明

### ⚠️ 重要变更

**不向前兼容**：网络流量统计功能的配置文件格式已完全改变，从旧版本升级需要删除旧配置文件。

升级步骤：
```bash
# 备份旧配置（可选）
sudo cp /etc/komari-network.conf /etc/komari-network.conf.old

# 删除旧配置
sudo rm /etc/komari-network.conf

# 重启程序，自动创建新配置
sudo systemctl restart komari-monitor
```

## 近期更新

### Windows Toast Notify

由于安全性问题，现在默认情况下在 Windows 系统下运行时会发送 Windows 系统 Toast 通知，内容为:

```
Komari-monitor-rs Is Running!
Komari-monitor-rs is an application used to monitor your system, granting it near-complete access to your computer. If you did not actively install this program, please check your system immediately. If you have intentionally used this software on your system, please ignore this message or add `--disable-toast-notify` to your startup parameters.
```

可以通过 `--disable-toast-notify` 参数关闭

### Dry Run 支持

现在可以不提供任何参数，仅提供 `--dry-run` 参数，以事先获取监控数据

每次正常运行前也将获取一次数据，若有误监控的项目请发送 DryRun 的输出到 Issue 中，比如各种不应该读取的硬盘、虚拟网卡等

```
The following is the equipment that will be put into operation and monitored:
CPU: AMD EPYC 7763 64-Core Processor, Cores: 4
Memory: 2092 MB / 16773 MB
Swap: 0 MB / 0 MB
Load: 0.36 / 0.65 / 0.37

Hard drives will be monitored:
/dev/root | ext4 | /usr/sbin/docker-init | 8 GB / 31 GB

Network interfaces will be monitored:
eth0 | 00:22:48:58:ca:62 | UP: 0 GB / DOWN: 7 GB
CONNS: TCP: 12 | UDP: 4
```

### 流量统计功能 (已重构)

本项目已将流量统计功能从**周期清零模式**重构为**按月重置模式**，提供更符合 VPS 计费周期的流量统计方式。

主要特性：
- **按月重置**：在每月固定日期自动重置流量统计（默认每月 1 号）
- **流量校准**：支持设置基准值，与服务商流量对齐
- **配置热重载**：修改配置无需重启程序
- **智能月末处理**：自动适配不同月份的天数

详细使用方法请参考：[NETWORK_RESET_GUIDE.md](NETWORK_RESET_GUIDE.md)

## 一键安装脚本

推荐使用 `kagent.sh` 脚本进行安装和管理：

```bash
# 下载并执行安装脚本
curl -O https://raw.githubusercontent.com/NEOherozzz/komari-monitor-rs/main/kagent.sh
chmod +x kagent.sh

# 交互式安装
sudo ./kagent.sh install

# 无交互安装（适用于自动化部署）
sudo ./kagent.sh install --non-interactive --http-server "https://your-server.com" --token "your-token"
```

脚本支持的操作：
- `install` - 安装或更新 komari-monitor-rs
- `uninstall` - 卸载服务
- `start/stop/restart` - 服务控制
- `status` - 查看运行状态
- `logs` - 查看日志

## 与原版的差异

目前，本项目已经实现原版的大部分功能，但还有以下的差异:

- GPU Name 检测

除此之外，还有希望添加的功能:

- 自动更新
- ~~自动安装~~ ✅ 已实现 (kagent.sh)
- ~~Bash / PWSH 一键脚本~~ ✅ 已实现 (kagent.sh)

## 下载

在本项目的 [Release 界面](https://github.com/NEOherozzz/komari-monitor-rs/releases) 即可下载，按照架构选择即可

后缀有 `musl` 字样的可以在任何 Linux 系统下运行

后缀有 `gnu` 字样的仅可以在较新的，通用的，带有 `Glibc` 的 Linux 系统下运行，占用会小一些

## Usage

```
komari-monitor-rs is a third-party high-performance monitoring agent for the komari monitoring service.

Usage: komari-monitor-rs [OPTIONS]

Options:
      --http-server <HTTP_SERVER>
          Set Main Server Http Address

      --ws-server <WS_SERVER>
          Set Main Server WebSocket Address

  -t, --token <TOKEN>
          Set Token

  -f, --fake <FAKE>
          Set Fake Multiplier
          [default: 1]

      --tls
          Enable TLS (default disabled)
          [default: false]

      --ignore-unsafe-cert
          Ignore Certificate Verification
          [default: false]

  -d, --dry-run
          Dry Run
          [default: false]

      --log-level <LOG_LEVEL>
          Set Log Level (Enable Debug or Trace for issue reporting)
          [default: info]

      --ip-provider <IP_PROVIDER>
          Public IP Provider
          [default: ipinfo]

      --terminal
          Enable Terminal (default disabled)
          [default: false]

      --terminal-entry <TERMINAL_ENTRY>
          Custom Terminal Entry
          [default: default]

      --realtime-info-interval <REALTIME_INFO_INTERVAL>
          Set Real-Time Info Upload Interval (ms)
          [default: 1000]

      --disable-toast-notify
          Disable Windows Toast Notification (Only Windows)
          [default: false]

      --disable-network-statistics
          Disable Network Statistics
          [default: false]

      --network-interval <NETWORK_INTERVAL>
          Network Statistics Interval (s)
          [default: 10]

      --reset-day <RESET_DAY>
          Monthly reset day (1-31, auto-adjusts for month-end)
          [default: 1]

      --calibration-tx <CALIBRATION_TX>
          Upload traffic calibration value (bytes)
          [default: 0]

      --calibration-rx <CALIBRATION_RX>
          Download traffic calibration value (bytes)
          [default: 0]

      --network-save-path <NETWORK_SAVE_PATH>
          Network Statistics Save Path
```

必须设置 `--http-server` / `--token`
`--ip-provider` 接受 `cloudflare` / `ipinfo`
`--log-level` 接受 `error`, `warn`, `info`, `debug`, `trace`

## Nix 安装

如果你使用 Nix / NixOS，可以直接将本仓库作为 Flake 引入使用：

> [!WARNING]
> 以下是最小化示例配置，单独使用无法工作

```nix
{
  # 将 komari-monitor-rs 作为 flake 引入
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    komari-monitor-rs = {
      url = "github:GenshinMinecraft/komari-monitor-rs";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { nixpkgs, komari-monitor-rs, ... }: {
    nixosConfigurations."nixos" = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        komari-monitor-rs.nixosModules.default
        { pkgs, ...}: {
          # 开启并配置 komari-monitor-rs 服务
          services.komari-monitor-rs = {
            enable = true;
            settings = {
              http-server = "https://komari.example.com:12345";
              ws-server = "ws://ws-komari.example.com:54321";
              token = "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
              ip-provider = "ipinfo";
              terminal = true;
              terminal-entry = "default";
              fake = 1;
              realtime-info-interval = 1000;
              tls = true;
              ignore-unsafe-cert = false;
              log-level = "info";
            };
          };
        }
      ];
    };
  };
}
```

## LICENSE

本项目根据 WTFPL 许可证开源

```
        DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE 
                    Version 2, December 2004 

 Copyright (C) 2004 Sam Hocevar <sam@hocevar.net> 

 Everyone is permitted to copy and distribute verbatim or modified 
 copies of this license document, and changing it is allowed as long 
 as the name is changed. 

            DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE 
   TERMS AND CONDITIONS FOR COPYING, DISTRIBUTION AND MODIFICATION 

  0. You just DO WHAT THE FUCK YOU WANT TO.
```
