#!/bin/bash

#================================================================================
# Komari Agent 管理工具
#
# 功能:
#   - install      安装 Komari Agent
#   - uninstall    卸载 Komari Agent
#   - start        启动服务
#   - stop         停止服务
#   - restart      重启服务
#   - status       查看服务状态
#   - logs         查看实时日志
#   - config       配置管理 (show/edit/set)
#   - update       更新程序到最新版本
#   - help         显示帮助信息
#
# 使用方法:
#   sudo bash kagent.sh install [参数]
#   sudo bash kagent.sh config set <key> <value>
#   sudo bash kagent.sh status
#================================================================================

# --- 配置常量 ---
GITHUB_REPO="NEOherozzz/komari-monitor-rs"
INSTALL_PATH="/usr/local/bin/komari-monitor-rs"
SERVICE_NAME="komari-agent-rs"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"
CONFIG_FILE="/etc/komari-agent.conf"
NETWORK_DATA_DIR="/var/lib/komari-monitor"
NETWORK_DATA_FILE="${NETWORK_DATA_DIR}/network-data.conf"
VERSION="1.0.0"

# --- 颜色定义 ---
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# --- 日志函数 ---
log_info() {
    echo -e "${GREEN}[信息]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[警告]${NC} $1"
}

log_error() {
    echo -e "${RED}[错误]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[成功]${NC} $1"
}

log_header() {
    echo -e "${CYAN}========================================${NC}"
    echo -e "${CYAN}$1${NC}"
    echo -e "${CYAN}========================================${NC}"
}

# --- 权限检查 ---
check_root() {
    if [ "$(id -u)" -ne 0 ]; then
        log_error "此操作需要 root 权限，请使用 'sudo' 运行"
        exit 1
    fi
}

# --- 依赖安装 ---
install_dependencies() {
    if command -v wget &> /dev/null; then
        log_info "依赖 'wget' 已安装"
        return
    fi

    log_info "正在安装 'wget'..."
    if command -v apt-get &> /dev/null; then
        apt-get update && apt-get install -y wget
    elif command -v yum &> /dev/null; then
        yum install -y wget
    elif command -v dnf &> /dev/null; then
        dnf install -y wget
    elif command -v pacman &> /dev/null; then
        pacman -Sy --noconfirm wget
    else
        log_error "未找到支持的包管理器，请手动安装 'wget'"
        exit 1
    fi

    if ! command -v wget &> /dev/null; then
        log_error "安装 'wget' 失败"
        exit 1
    fi
    log_success "'wget' 安装成功"
}

# --- 架构检测 ---
get_arch() {
    ARCH=$(uname -m)
    case $ARCH in
        x86_64)
            echo "komari-monitor-rs-linux-x86_64-gnu"
            ;;
        i686)
            echo "komari-monitor-rs-linux-i686-gnu"
            ;;
        aarch64)
            echo "komari-monitor-rs-linux-aarch64-gnu"
            ;;
        armv7l)
            echo "komari-monitor-rs-linux-armv7-gnueabihf"
            ;;
        armv5tejl)
            echo "komari-monitor-rs-linux-armv5te-gnueabi"
            ;;
        *)
            log_error "不支持的系统架构: $ARCH"
            log_error "请手动下载: https://github.com/${GITHUB_REPO}/releases/latest"
            exit 1
            ;;
    esac
}

#================================================================================
# 网络数据初始化
#================================================================================
# 获取当前系统的网络流量（应用过滤规则，排除虚拟接口）
get_current_network_traffic() {
    local total_tx=0
    local total_rx=0

    # 过滤关键词（与 Rust 代码中的 FILTER_KEYWORDS 一致）
    local filter_keywords=("br" "cni" "docker" "podman" "flannel" "lo" "veth" "virbr" "vmbr" "tap" "tun" "fwln" "fwpr")

    # 遍历所有网络接口
    for interface in /sys/class/net/*; do
        if [ ! -d "$interface" ]; then
            continue
        fi

        local if_name=$(basename "$interface")
        local should_filter=false

        # 检查是否应该过滤此接口
        for keyword in "${filter_keywords[@]}"; do
            if [[ "$if_name" == *"$keyword"* ]]; then
                should_filter=true
                break
            fi
        done

        # 跳过被过滤的接口
        if [ "$should_filter" = true ]; then
            continue
        fi

        # 读取流量统计（如果文件存在）
        if [ -f "$interface/statistics/tx_bytes" ] && [ -f "$interface/statistics/rx_bytes" ]; then
            local tx=$(cat "$interface/statistics/tx_bytes" 2>/dev/null || echo "0")
            local rx=$(cat "$interface/statistics/rx_bytes" 2>/dev/null || echo "0")

            # 确保是数字
            if [[ "$tx" =~ ^[0-9]+$ ]] && [[ "$rx" =~ ^[0-9]+$ ]]; then
                total_tx=$((total_tx + tx))
                total_rx=$((total_rx + rx))
            fi
        fi
    done

    echo "$total_tx $total_rx"
}

# 计算 last_reset_month
calculate_last_reset_month() {
    local reset_day=$1
    local current_month=$(date +%-m)  # 1-12
    local current_day=$(date +%-d)    # 1-31

    # 获取当前月份的天数
    local days_in_month=$(date -d "$(date +%Y-%m-01) +1 month -1 day" +%-d)

    # 计算有效的 reset_day（如果超过当月天数，使用当月最后一天）
    local effective_reset_day=$reset_day
    if [ $reset_day -gt $days_in_month ]; then
        effective_reset_day=$days_in_month
    fi

    # 如果当前日期 >= reset_day，则 last_reset_month = current_month
    # 否则 last_reset_month = 上个月
    if [ $current_day -ge $effective_reset_day ]; then
        echo $current_month
    else
        # 上个月
        if [ $current_month -eq 1 ]; then
            echo 12
        else
            echo $((current_month - 1))
        fi
    fi
}

# 初始化网络数据文件
init_network_data() {
    local reset_day=$1

    # 获取当前系统流量
    local traffic=$(get_current_network_traffic)
    local current_tx=$(echo $traffic | cut -d' ' -f1)
    local current_rx=$(echo $traffic | cut -d' ' -f2)

    # 获取 boot_id（Linux）
    local boot_id=""
    if [ -f "/proc/sys/kernel/random/boot_id" ]; then
        boot_id=$(cat /proc/sys/kernel/random/boot_id 2>/dev/null | tr -d '\n')
    fi

    # 计算 last_reset_month
    local last_reset_month=$(calculate_last_reset_month "$reset_day")

    # 创建网络数据文件
    cat > "${NETWORK_DATA_FILE}" <<EOF
# Komari Monitor Runtime Data
# This file is automatically managed by the program. Do not modify manually.

boot_id=${boot_id}
boot_source_tx=${current_tx}
boot_source_rx=${current_rx}
current_boot_tx=0
current_boot_rx=0
accumulated_tx=0
accumulated_rx=0
last_reset_month=${last_reset_month}
EOF

    log_success "网络数据文件已初始化: ${NETWORK_DATA_FILE}"
    log_info "  当前系统上传流量: ${current_tx} 字节"
    log_info "  当前系统下载流量: ${current_rx} 字节"
    log_info "  流量统计将从此刻开始计算"
}

#================================================================================
# 安装功能
#================================================================================
cmd_install() {
    check_root
    log_header "开始安装 Komari Agent"

    # 解析参数
    HTTP_SERVER=""
    TOKEN=""
    FAKE="1"
    INTERVAL="1000"
    TLS_FLAG=""
    IGNORE_CERT_FLAG=""
    TERMINAL_FLAG="unset"  # 使用 "unset" 标记未设置状态
    RESET_DAY=""
    CALIBRATION_TX=""
    CALIBRATION_RX=""
    TRAFFIC_MODE=""

    while [ "$#" -gt 0 ]; do
        case "$1" in
            --http-server) HTTP_SERVER="$2"; shift 2;;
            -t|--token) TOKEN="$2"; shift 2;;
            -f|--fake) FAKE="$2"; shift 2;;
            --realtime-info-interval) INTERVAL="$2"; shift 2;;
            --tls) TLS_FLAG="--tls"; shift 1;;
            --ignore-unsafe-cert) IGNORE_CERT_FLAG="--ignore-unsafe-cert"; shift 1;;
            --terminal) TERMINAL_FLAG="$2"; shift 2;;
            --reset-day) RESET_DAY="$2"; shift 2;;
            --calibration-tx) CALIBRATION_TX="$2"; shift 2;;
            --calibration-rx) CALIBRATION_RX="$2"; shift 2;;
            --traffic-mode) TRAFFIC_MODE="$2"; shift 2;;
            *) log_warn "未知参数: $1"; shift 1;;
        esac
    done

    # 交互式询问
    if [ -z "$HTTP_SERVER" ]; then
        read -p "请输入主端 HTTP 地址 (例如 http://127.0.0.1:8080): " HTTP_SERVER
    fi
    if [ -z "$TOKEN" ]; then
        read -p "请输入 Token: " TOKEN
    fi

    if [ "$TERMINAL_FLAG" = "unset" ]; then
        read -p "是否启用 Web Terminal 功能? (y/N): " enable_terminal
        enable_terminal_lower=$(echo "$enable_terminal" | tr '[:upper:]' '[:lower:]')
        if [[ "$enable_terminal_lower" == "y" || "$enable_terminal_lower" == "yes" ]]; then
            TERMINAL_FLAG="true"
        else
            TERMINAL_FLAG="false"
        fi
    fi

    # 询问网络流量统计配置
    if [ -z "$RESET_DAY" ]; then
        read -p "请输入流量重置日期 [1-31] (默认: 1): " RESET_DAY
        RESET_DAY=${RESET_DAY:-1}
        # 验证 reset_day 范围
        if ! [[ "$RESET_DAY" =~ ^[0-9]+$ ]] || [ "$RESET_DAY" -lt 1 ] || [ "$RESET_DAY" -gt 31 ]; then
            log_warn "流量重置日期无效，使用默认值: 1"
            RESET_DAY=1
        fi
    fi

    if [ -z "$CALIBRATION_TX" ]; then
        read -p "请输入上传流量校准值（字节）(默认: 0): " CALIBRATION_TX
        CALIBRATION_TX=${CALIBRATION_TX:-0}
        # 验证为数字
        if ! [[ "$CALIBRATION_TX" =~ ^[0-9]+$ ]]; then
            log_warn "上传流量校准值无效，使用默认值: 0"
            CALIBRATION_TX=0
        fi
    fi

    if [ -z "$CALIBRATION_RX" ]; then
        read -p "请输入下载流量校准值（字节）(默认: 0): " CALIBRATION_RX
        CALIBRATION_RX=${CALIBRATION_RX:-0}
        # 验证为数字
        if ! [[ "$CALIBRATION_RX" =~ ^[0-9]+$ ]]; then
            log_warn "下载流量校准值无效，使用默认值: 0"
            CALIBRATION_RX=0
        fi
    fi

    if [ -z "$TRAFFIC_MODE" ]; then
        echo ""
        echo "请选择流量统计模式:"
        echo "  1) both     - 双向统计 (上传 + 下载)"
        echo "  2) tx_only  - 仅统计上传流量 (适用于只计费出站流量的VPS)"
        echo "  3) rx_only  - 仅统计下载流量 (适用于只计费入站流量的VPS)"
        read -p "请输入选项 [1-3] (默认: 1): " traffic_mode_choice
        traffic_mode_choice=${traffic_mode_choice:-1}

        case "$traffic_mode_choice" in
            1) TRAFFIC_MODE="both" ;;
            2) TRAFFIC_MODE="tx_only" ;;
            3) TRAFFIC_MODE="rx_only" ;;
            *)
                log_warn "流量统计模式选择无效，使用默认值: both"
                TRAFFIC_MODE="both"
                ;;
        esac
    fi

    # 验证输入
    if [ -z "$HTTP_SERVER" ] || [ -z "$TOKEN" ]; then
        log_error "HTTP 地址和 Token 不能为空"
        exit 1
    fi

    log_info "配置信息确认:"
    echo "  Http Server: $HTTP_SERVER"
    echo "  Token: ********"
    echo "  虚假倍率: $FAKE"
    echo "  上传间隔: $INTERVAL ms"
    echo "  启用 TLS: ${TLS_FLAG:--}"
    echo "  忽略证书: ${IGNORE_CERT_FLAG:--}"
    echo "  启用 Terminal: $TERMINAL_FLAG"
    echo "  流量重置日期: 每月 $RESET_DAY 号"
    echo "  上传流量校准: $CALIBRATION_TX 字节"
    echo "  下载流量校准: $CALIBRATION_RX 字节"
    echo "  流量统计模式: $TRAFFIC_MODE"
    echo ""

    # 安装依赖
    install_dependencies

    # 检查服务是否已存在并运行
    SERVICE_WAS_RUNNING=false
    if systemctl list-unit-files | grep -q "${SERVICE_NAME}"; then
        if systemctl is-active --quiet ${SERVICE_NAME}; then
            log_warn "检测到服务正在运行，需要先停止服务"
            systemctl stop ${SERVICE_NAME}
            SERVICE_WAS_RUNNING=true
            log_info "服务已停止"
            sleep 1
        fi
    fi

    # 下载程序
    ARCH_FILE=$(get_arch)
    DOWNLOAD_URL="https://ghfast.top/https://github.com/${GITHUB_REPO}/releases/download/latest/${ARCH_FILE}"

    log_info "检测到系统架构: $(uname -m)"
    log_info "正在下载: ${DOWNLOAD_URL}"

    # 如果文件已存在且正在使用，先备份
    if [ -f "${INSTALL_PATH}" ]; then
        cp "${INSTALL_PATH}" "${INSTALL_PATH}.backup" 2>/dev/null || true
    fi

    if ! wget -O "${INSTALL_PATH}" "${DOWNLOAD_URL}"; then
        log_error "下载失败，请检查网络连接"
        # 恢复备份
        if [ -f "${INSTALL_PATH}.backup" ]; then
            mv "${INSTALL_PATH}.backup" "${INSTALL_PATH}"
            log_info "已恢复原程序"
            if [ "$SERVICE_WAS_RUNNING" = true ]; then
                systemctl start ${SERVICE_NAME}
            fi
        fi
        exit 1
    fi

    # 删除备份
    rm -f "${INSTALL_PATH}.backup"

    chmod +x "${INSTALL_PATH}"
    log_success "程序已安装到: ${INSTALL_PATH}"

    # 检查是否存在旧配置文件
    CONFIG_EXISTS=false
    if [ -f "${CONFIG_FILE}" ]; then
        CONFIG_EXISTS=true
        log_warn "检测到已存在的配置文件: ${CONFIG_FILE}"
        read -p "是否保留现有配置? (Y/n): " keep_config
        keep_config_lower=$(echo "$keep_config" | tr '[:upper:]' '[:lower:]')

        if [[ "$keep_config_lower" != "n" && "$keep_config_lower" != "no" ]]; then
            log_info "保留现有配置文件"
            # 跳过配置文件创建，直接跳到服务重启
            systemctl daemon-reload
            if [ "$SERVICE_WAS_RUNNING" = true ]; then
                systemctl start ${SERVICE_NAME}
                sleep 2
                if systemctl is-active --quiet ${SERVICE_NAME}; then
                    log_success "服务已成功重启并正在运行"
                else
                    log_error "服务启动失败，请查看日志: sudo journalctl -u ${SERVICE_NAME}"
                    exit 1
                fi
            else
                systemctl enable ${SERVICE_NAME}
                systemctl start ${SERVICE_NAME}
                sleep 2
                if systemctl is-active --quiet ${SERVICE_NAME}; then
                    log_success "服务已成功启动并正在运行"
                else
                    log_error "服务启动失败，请查看日志: sudo journalctl -u ${SERVICE_NAME}"
                    exit 1
                fi
            fi
            echo ""
            log_info "常用命令:"
            echo "  查看状态: sudo kagent.sh status"
            echo "  查看日志: sudo kagent.sh logs"
            echo "  修改配置: sudo kagent.sh config edit"
            return
        else
            log_info "将使用新配置覆盖现有配置"
            cp "${CONFIG_FILE}" "${CONFIG_FILE}.backup"
            log_info "已备份旧配置到: ${CONFIG_FILE}.backup"
        fi
    fi

    # 创建配置文件（按照 komari-agent.conf.example 格式）
    cat > ${CONFIG_FILE} <<EOF
# Komari Monitor Agent Configuration
# Generated at $(date)

# ==================== Main Server Configuration ====================
# REQUIRED: HTTP server address
http_server=${HTTP_SERVER}

# OPTIONAL: WebSocket server address (if not specified, will be converted from http_server)
ws_server=

# REQUIRED: Authentication token
token=${TOKEN}

# ==================== TLS Configuration ====================
# Enable TLS encryption for connections (default: false)
tls=$(if [ -n "${TLS_FLAG}" ]; then echo "true"; else echo "false"; fi)

# Ignore unsafe/self-signed certificates (default: false)
# WARNING: Only use this for testing purposes
ignore_unsafe_cert=$(if [ -n "${IGNORE_CERT_FLAG}" ]; then echo "true"; else echo "false"; fi)

# ==================== Performance Configuration ====================
# Fake multiplier for system metrics (default: 1.0)
# Use values > 1.0 to artificially inflate metrics (for testing)
fake=${FAKE}

# Real-time information upload interval in milliseconds (default: 1000)
# How often to send system metrics to the server
realtime_info_interval=${INTERVAL}

# ==================== Feature Configuration ====================
# Public IP address provider (default: ipinfo)
# Options: cloudflare, ipinfo
ip_provider=ipinfo

# Enable web terminal feature (default: false)
# Allows remote command execution via web interface
# WARNING: This is a security-sensitive feature
terminal=${TERMINAL_FLAG}

# Terminal entry program (default: default)
# default = auto-detect (cmd.exe on Windows, bash or sh on Linux)
# Or specify a custom shell like: /bin/zsh, /bin/fish, etc.
terminal_entry=default

# Disable Windows toast notifications (default: false)
# Only applicable on Windows systems
disable_toast_notify=false

# ==================== Network Statistics Configuration ====================
# Disable network traffic statistics (default: false)
# Set to true to disable traffic monitoring completely
disable_network_statistics=false

# Network statistics sampling interval in seconds (default: 10)
# How often to sample network interface traffic
network_interval=10

# Day of month to reset traffic statistics (default: 1)
# Valid range: 1-31
# If the day exceeds the month's days (e.g., 31 in February), uses last day of month
reset_day=${RESET_DAY}

# Traffic calibration for upload in bytes (default: 0)
# Use this to align with your VPS provider's traffic statistics
calibration_tx=${CALIBRATION_TX}

# Traffic calibration for download in bytes (default: 0)
calibration_rx=${CALIBRATION_RX}

# Traffic counting mode (default: both)
# Options: both, tx_only, rx_only
traffic_mode=${TRAFFIC_MODE}

# ==================== Logging Configuration ====================
# Log level (default: info)
# Options: error, warn, info, debug, trace
log_level=info

# ==================== Notes ====================
# 1. After modifying this file, restart the service for changes to take effect:
#    sudo systemctl restart ${SERVICE_NAME}
#
# 2. View service status:
#    sudo kagent.sh status
#
# 3. View logs:
#    sudo journalctl -u ${SERVICE_NAME} -f
#
# 4. For more information, visit:
#    https://github.com/${GITHUB_REPO}
EOF

    log_success "配置文件已创建: ${CONFIG_FILE}"

    # 创建 network-data 目录
    if [ ! -d "${NETWORK_DATA_DIR}" ]; then
        mkdir -p "${NETWORK_DATA_DIR}"
        log_info "已创建数据目录: ${NETWORK_DATA_DIR}"
    fi

    # 初始化网络数据文件（仅在不存在时）
    if [ ! -f "${NETWORK_DATA_FILE}" ]; then
        log_info "正在初始化网络流量统计..."
        init_network_data "${RESET_DAY}"
    else
        log_info "检测到已存在的网络数据文件，保留现有数据"
    fi

    # 创建 systemd 服务
    cat > ${SERVICE_FILE} <<EOF
[Unit]
Description=Komari Monitor RS Service
After=network.target

[Service]
Type=simple
User=root
ExecStart=${INSTALL_PATH} --config ${CONFIG_FILE}
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

    log_success "systemd 服务文件已创建"

    # 启动服务
    systemctl daemon-reload
    systemctl enable ${SERVICE_NAME}
    systemctl start ${SERVICE_NAME}

    sleep 2
    if systemctl is-active --quiet ${SERVICE_NAME}; then
        log_success "服务已成功启动并正在运行"
        echo ""
        log_info "常用命令:"
        echo "  查看状态: sudo kagent.sh status"
        echo "  查看日志: sudo kagent.sh logs"
        echo "  修改配置: sudo kagent.sh config edit"
    else
        log_error "服务启动失败，请查看日志: sudo journalctl -u ${SERVICE_NAME}"
        exit 1
    fi
}

#================================================================================
# 卸载功能
#================================================================================
cmd_uninstall() {
    check_root
    log_header "开始卸载 Komari Agent"

    read -p "确认要卸载 Komari Agent 吗? (y/N): " confirm
    confirm_lower=$(echo "$confirm" | tr '[:upper:]' '[:lower:]')

    if [[ "$confirm_lower" != "y" && "$confirm_lower" != "yes" ]]; then
        log_info "取消卸载"
        exit 0
    fi

    # 停止并禁用服务
    if systemctl is-active --quiet ${SERVICE_NAME}; then
        log_info "正在停止服务..."
        systemctl stop ${SERVICE_NAME}
    fi

    if systemctl is-enabled --quiet ${SERVICE_NAME} 2>/dev/null; then
        log_info "正在禁用服务..."
        systemctl disable ${SERVICE_NAME}
    fi

    # 删除服务文件
    if [ -f "${SERVICE_FILE}" ]; then
        rm -f "${SERVICE_FILE}"
        log_success "已删除服务文件: ${SERVICE_FILE}"
    fi

    # 删除程序
    if [ -f "${INSTALL_PATH}" ]; then
        rm -f "${INSTALL_PATH}"
        log_success "已删除程序: ${INSTALL_PATH}"
    fi

    # 询问是否删除配置文件和数据文件
    read -p "是否删除配置文件和数据文件? (y/N): " delete_config
    delete_config_lower=$(echo "$delete_config" | tr '[:upper:]' '[:lower:]')

    if [[ "$delete_config_lower" == "y" || "$delete_config_lower" == "yes" ]]; then
        # 删除配置文件
        if [ -f "${CONFIG_FILE}" ]; then
            rm -f "${CONFIG_FILE}"
            log_success "已删除配置文件: ${CONFIG_FILE}"
        fi

        # 删除 network-data 文件
        if [ -f "${NETWORK_DATA_FILE}" ]; then
            rm -f "${NETWORK_DATA_FILE}"
            log_success "已删除数据文件: ${NETWORK_DATA_FILE}"
        fi

        # 删除数据目录（如果为空）
        if [ -d "${NETWORK_DATA_DIR}" ] && [ -z "$(ls -A ${NETWORK_DATA_DIR})" ]; then
            rmdir "${NETWORK_DATA_DIR}"
            log_success "已删除数据目录: ${NETWORK_DATA_DIR}"
        fi
    else
        log_info "保留配置文件: ${CONFIG_FILE}"
        if [ -f "${NETWORK_DATA_FILE}" ]; then
            log_info "保留数据文件: ${NETWORK_DATA_FILE}"
        fi
    fi

    systemctl daemon-reload
    log_success "Komari Agent 已成功卸载"
}

#================================================================================
# 服务控制功能
#================================================================================
cmd_start() {
    check_root
    log_info "正在启动服务..."
    systemctl start ${SERVICE_NAME}

    sleep 1
    if systemctl is-active --quiet ${SERVICE_NAME}; then
        log_success "服务已启动"
    else
        log_error "服务启动失败"
        exit 1
    fi
}

cmd_stop() {
    check_root
    log_info "正在停止服务..."
    systemctl stop ${SERVICE_NAME}
    log_success "服务已停止"
}

cmd_restart() {
    check_root
    log_info "正在重启服务..."
    systemctl restart ${SERVICE_NAME}

    sleep 1
    if systemctl is-active --quiet ${SERVICE_NAME}; then
        log_success "服务已重启"
    else
        log_error "服务重启失败"
        exit 1
    fi
}

#================================================================================
# 状态查看功能
#================================================================================
cmd_status() {
    log_header "服务状态"

    if ! systemctl list-unit-files | grep -q "${SERVICE_NAME}"; then
        log_error "服务未安装"
        exit 1
    fi

    # 显示服务状态
    systemctl status ${SERVICE_NAME} --no-pager

    # 显示额外信息
    echo ""
    log_info "服务信息:"
    echo "  服务名称: ${SERVICE_NAME}"
    echo "  程序路径: ${INSTALL_PATH}"
    echo "  配置文件: ${CONFIG_FILE}"
    echo "  数据目录: ${NETWORK_DATA_DIR}"
    echo "  服务文件: ${SERVICE_FILE}"

    if [ -f "${CONFIG_FILE}" ]; then
        echo ""
        log_info "当前配置:"
        grep -E "^(http_server|token|terminal|network_interval|reset_day)" ${CONFIG_FILE} | while read line; do
            key=$(echo "$line" | cut -d'=' -f1)
            value=$(echo "$line" | cut -d'=' -f2-)
            if [ "$key" = "token" ]; then
                echo "  $key=********"
            else
                echo "  $line"
            fi
        done
    fi
}

#================================================================================
# 日志查看功能
#================================================================================
cmd_logs() {
    log_info "查看实时日志 (按 Ctrl+C 退出)..."
    echo ""
    journalctl -u ${SERVICE_NAME} -f
}

#================================================================================
# 配置管理功能
#================================================================================
cmd_config() {
    subcommand="${1:-show}"

    case "$subcommand" in
        show)
            if [ ! -f "${CONFIG_FILE}" ]; then
                log_error "配置文件不存在: ${CONFIG_FILE}"
                exit 1
            fi

            log_header "当前配置"
            cat ${CONFIG_FILE} | while read line; do
                if [[ "$line" =~ ^token= ]]; then
                    echo "token=********"
                else
                    echo "$line"
                fi
            done
            ;;

        edit)
            check_root
            if [ ! -f "${CONFIG_FILE}" ]; then
                log_error "配置文件不存在: ${CONFIG_FILE}"
                exit 1
            fi

            # 使用系统默认编辑器
            EDITOR="${EDITOR:-vi}"
            $EDITOR ${CONFIG_FILE}

            log_info "配置已修改，是否重启服务以应用更改? (y/N): "
            read restart_confirm
            restart_confirm_lower=$(echo "$restart_confirm" | tr '[:upper:]' '[:lower:]')

            if [[ "$restart_confirm_lower" == "y" || "$restart_confirm_lower" == "yes" ]]; then
                cmd_restart
            fi
            ;;

        set)
            check_root
            key="$2"
            value="$3"

            if [ -z "$key" ] || [ -z "$value" ]; then
                log_error "用法: kagent.sh config set <key> <value>"
                exit 1
            fi

            if [ ! -f "${CONFIG_FILE}" ]; then
                log_error "配置文件不存在: ${CONFIG_FILE}"
                exit 1
            fi

            # 检查键是否存在
            if grep -q "^${key}=" ${CONFIG_FILE}; then
                # 更新现有配置
                sed -i "s|^${key}=.*|${key}=${value}|" ${CONFIG_FILE}
                log_success "已更新配置: ${key}=${value}"
            else
                # 添加新配置
                echo "${key}=${value}" >> ${CONFIG_FILE}
                log_success "已添加配置: ${key}=${value}"
            fi

            log_info "是否重启服务以应用更改? (y/N): "
            read restart_confirm
            restart_confirm_lower=$(echo "$restart_confirm" | tr '[:upper:]' '[:lower:]')

            if [[ "$restart_confirm_lower" == "y" || "$restart_confirm_lower" == "yes" ]]; then
                cmd_restart
            fi
            ;;

        *)
            log_error "未知的配置命令: $subcommand"
            echo "可用命令: show, edit, set"
            exit 1
            ;;
    esac
}

#================================================================================
# 更新功能
#================================================================================
cmd_update() {
    check_root
    log_header "更新 Komari Agent"

    if [ ! -f "${INSTALL_PATH}" ]; then
        log_error "程序未安装，请先运行 'kagent.sh install'"
        exit 1
    fi

    # 下载新版本
    ARCH_FILE=$(get_arch)
    DOWNLOAD_URL="https://ghfast.top/https://github.com/${GITHUB_REPO}/releases/download/latest/${ARCH_FILE}"

    log_info "正在下载最新版本..."
    TEMP_FILE="/tmp/komari-monitor-rs.new"

    if ! wget -O "${TEMP_FILE}" "${DOWNLOAD_URL}"; then
        log_error "下载失败"
        rm -f "${TEMP_FILE}"
        exit 1
    fi

    chmod +x "${TEMP_FILE}"

    # 停止服务
    log_info "正在停止服务..."
    systemctl stop ${SERVICE_NAME}

    # 备份旧版本
    if [ -f "${INSTALL_PATH}" ]; then
        cp "${INSTALL_PATH}" "${INSTALL_PATH}.backup"
        log_info "已备份旧版本到: ${INSTALL_PATH}.backup"
    fi

    # 替换程序
    mv "${TEMP_FILE}" "${INSTALL_PATH}"
    log_success "程序已更新"

    # 重启服务
    log_info "正在重启服务..."
    systemctl start ${SERVICE_NAME}

    sleep 2
    if systemctl is-active --quiet ${SERVICE_NAME}; then
        log_success "服务已成功重启"
        rm -f "${INSTALL_PATH}.backup"
    else
        log_error "服务启动失败，正在恢复旧版本..."
        mv "${INSTALL_PATH}.backup" "${INSTALL_PATH}"
        systemctl start ${SERVICE_NAME}
        log_error "已恢复到旧版本"
        exit 1
    fi
}

#================================================================================
# 帮助信息
#================================================================================
cmd_help() {
    echo -e "${CYAN}Komari Agent 管理工具 v${VERSION}${NC}"
    echo ""
    echo -e "${GREEN}使用方法:${NC}"
    echo "  sudo kagent.sh <命令> [参数]"
    echo ""
    echo -e "${GREEN}可用命令:${NC}"
    echo -e "  ${YELLOW}install${NC}              安装 Komari Agent"
    echo "                       参数: --http-server, --token, --terminal <true|false>,"
    echo "                             --reset-day, --calibration-tx, --calibration-rx, --traffic-mode, 等"
    echo "                       示例: sudo kagent.sh install --http-server http://example.com:8080 --token mytoken --terminal false --reset-day 5 --traffic-mode both"
    echo ""
    echo -e "  ${YELLOW}uninstall${NC}            卸载 Komari Agent"
    echo ""
    echo -e "  ${YELLOW}start${NC}                启动服务"
    echo -e "  ${YELLOW}stop${NC}                 停止服务"
    echo -e "  ${YELLOW}restart${NC}              重启服务"
    echo ""
    echo -e "  ${YELLOW}status${NC}               查看服务状态和配置信息"
    echo ""
    echo -e "  ${YELLOW}logs${NC}                 查看实时日志"
    echo ""
    echo -e "  ${YELLOW}config show${NC}          显示当前配置"
    echo -e "  ${YELLOW}config edit${NC}          编辑配置文件"
    echo -e "  ${YELLOW}config set${NC} <key> <value>"
    echo "                       设置配置项"
    echo "                       示例: sudo kagent.sh config set reset_day 5"
    echo ""
    echo -e "  ${YELLOW}update${NC}               更新程序到最新版本"
    echo ""
    echo -e "  ${YELLOW}help${NC}                 显示此帮助信息"
    echo ""
    echo -e "${GREEN}常用配置项:${NC}"
    echo "  http_server              HTTP 服务器地址"
    echo "  token                    认证令牌"
    echo "  terminal                 启用 Web Terminal (true/false)"
    echo "  network_interval         网络统计采样间隔（秒）"
    echo "  reset_day                流量重置日期（1-31）"
    echo "  calibration_tx           上传流量校准值（字节）"
    echo "  calibration_rx           下载流量校准值（字节）"
    echo "  traffic_mode             流量统计模式（both/tx_only/rx_only）"
    echo "  log_level                日志级别（error/warn/info/debug/trace）"
    echo ""
    echo -e "${GREEN}文件位置:${NC}"
    echo "  程序:     ${INSTALL_PATH}"
    echo "  配置:     ${CONFIG_FILE}"
    echo "  数据:     ${NETWORK_DATA_DIR}"
    echo "  服务:     ${SERVICE_FILE}"
    echo ""
    echo -e "${GREEN}更多信息:${NC}"
    echo "  GitHub: https://github.com/${GITHUB_REPO}"
    echo ""
}

#================================================================================
# 主程序
#================================================================================
main() {
    command="${1:-help}"
    shift 2>/dev/null || true

    case "$command" in
        install)
            cmd_install "$@"
            ;;
        uninstall)
            cmd_uninstall "$@"
            ;;
        start)
            cmd_start "$@"
            ;;
        stop)
            cmd_stop "$@"
            ;;
        restart)
            cmd_restart "$@"
            ;;
        status)
            cmd_status "$@"
            ;;
        logs)
            cmd_logs "$@"
            ;;
        config)
            cmd_config "$@"
            ;;
        update)
            cmd_update "$@"
            ;;
        help|--help|-h)
            cmd_help
            ;;
        *)
            log_error "未知命令: $command"
            echo "运行 'kagent.sh help' 查看帮助信息"
            exit 1
            ;;
    esac
}

# 执行主程序
main "$@"
