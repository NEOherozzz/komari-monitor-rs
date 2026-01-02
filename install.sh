#!/bin/bash

#================================================================================
# Komari Monitor RS 安装脚本
#
# 功能:
#   - 检查 Root 权限
#   - 自动安装依赖 (wget)
#   - 自动检测系统架构并下载对应程序
#   - 通过命令行参数或交互式提问配置程序
#   - 创建并启用 systemd 服务实现后台保活和开机自启
#
# 使用方法:
#   1. 直接运行: bash install.sh
#   2. 带参数运行:
#      bash install.sh --http-server "http://your.server:port" --token "your_token" [--terminal] [--network-reset-day 1] [--network-calibration-tx 0] [--network-calibration-rx 0]
#================================================================================

# --- 配置 ---
# GitHub 仓库信息
GITHUB_REPO="NEOherozzz/komari-monitor-rs"
# 安装路径
INSTALL_PATH="/usr/local/bin/komari-monitor-rs"
# 服务名称
SERVICE_NAME="komari-agent-rs"
# systemd 服务文件路径
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"
# 网络配置文件路径
NETWORK_CONFIG_FILE="/etc/komari-network.conf"

# --- 颜色定义 ---
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# --- 日志函数 ---
log_info() {
    echo -e "${GREEN}[信息] $1${NC}"
}

log_warn() {
    echo -e "${YELLOW}[警告] $1${NC}"
}

log_error() {
    echo -e "${RED}[错误] $1${NC}"
}

# --- 脚本核心函数 ---

# 1. 检查是否以 Root 用户运行
check_root() {
    if [ "$(id -u)" -ne 0 ]; then
        log_error "此脚本需要 root 权限才能创建 systemd 服务。请使用 'sudo bash install.sh' 运行。"
        exit 1
    fi
}

# 2. 安装必要的依赖 (wget)
install_dependencies() {
    if command -v wget &> /dev/null; then
        log_info "依赖 'wget' 已安装。"
        return
    fi

    log_info "正在尝试安装 'wget'..."
    if command -v apt-get &> /dev/null; then
        apt-get update && apt-get install -y wget
    elif command -v yum &> /dev/null; then
        yum install -y wget
    elif command -v dnf &> /dev/null; then
        dnf install -y wget
    elif command -v pacman &> /dev/null; then
        pacman -Sy --noconfirm wget
    else
        log_error "未找到支持的包管理器 (apt/yum/dnf/pacman)。请手动安装 'wget' 后再运行此脚本。"
        exit 1
    fi

    if ! command -v wget &> /dev/null; then
        log_error "安装 'wget' 失败。请检查您的包管理器配置。"
        exit 1
    fi
    log_info "'wget' 安装成功。"
}

# 3. 检测系统架构
get_arch() {
    ARCH=$(uname -m)
    case $ARCH in
        x86_64)
            # 优先选择 gnu，因为通用性更强
            echo "komari-monitor-rs-linux-x86_64-gnu"
            ;;
        i686)
            echo "komari-monitor-rs-linux-i686-gnu"
            ;;
        aarch64)
            echo "komari-monitor-rs-linux-aarch64-gnu"
            ;;
        armv7l)
            # armv7l 通常对应 armv7-gnueabihf
            echo "komari-monitor-rs-linux-armv7-gnueabihf"
            ;;
        armv5tejl)
            echo "komari-monitor-rs-linux-armv5te-gnueabi"
            ;;
        *)
            log_error "不支持的系统架构: $ARCH"
            log_error "请从以下列表中手动选择并下载: https://github.com/${GITHUB_REPO}/releases/latest"
            exit 1
            ;;
    esac
}

# --- 主程序 ---
main() {
    check_root
    log_info "Komari Monitor RS 安装程序已启动。"

    # --- 参数初始化 ---
    HTTP_SERVER=""
    TOKEN=""
    FAKE="1"
    INTERVAL="1000"
    TLS_FLAG=""
    IGNORE_CERT_FLAG=""
    TERMINAL_FLAG=""
    # 网络统计配置
    NETWORK_RESET_DAY="1"
    NETWORK_CALIBRATION_TX="0"
    NETWORK_CALIBRATION_RX="0"
    DISABLE_NETWORK_STATS="false"

    # --- 解析命令行参数 ---
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --http-server) HTTP_SERVER="$2"; shift 2;;
            -t|--token) TOKEN="$2"; shift 2;;
            -f|--fake) FAKE="$2"; shift 2;;
            --realtime-info-interval) INTERVAL="$2"; shift 2;;
            --tls) TLS_FLAG="--tls"; shift 1;;
            --ignore-unsafe-cert) IGNORE_CERT_FLAG="--ignore-unsafe-cert"; shift 1;;
            --terminal) TERMINAL_FLAG="--terminal"; shift 1;;
            --network-reset-day) NETWORK_RESET_DAY="$2"; shift 2;;
            --network-calibration-tx) NETWORK_CALIBRATION_TX="$2"; shift 2;;
            --network-calibration-rx) NETWORK_CALIBRATION_RX="$2"; shift 2;;
            --disable-network-statistics) DISABLE_NETWORK_STATS="true"; shift 1;;
            *) log_warn "未知的参数: $1"; shift 1;;
        esac
    done

    # --- 交互式询问缺失的必要参数 ---
    if [ -z "$HTTP_SERVER" ]; then
        read -p "请输入主端 Http 地址 (例如 http://127.0.0.1:8080): " HTTP_SERVER
    fi
    if [ -z "$TOKEN" ]; then
        read -p "请输入 Token: " TOKEN
    fi

    # 交互式询问 --terminal (仅当命令行未提供时)
    if [ -z "$TERMINAL_FLAG" ]; then
      read -p "是否启用 Web Terminal 功能? (y/N): " enable_terminal
      enable_terminal_lower=$(echo "$enable_terminal" | tr '[:upper:]' '[:lower:]')
      if [[ "$enable_terminal_lower" == "y" || "$enable_terminal_lower" == "yes" ]]; then
          TERMINAL_FLAG="--terminal"
          log_info "Web Terminal 功能已启用。"
      else
          log_info "Web Terminal 功能未启用。"
      fi
    fi

    # 交互式询问网络统计配置
    read -p "每月流量重置日 (1-31，默认: 1): " input_reset_day
    if [ -n "$input_reset_day" ]; then
        NETWORK_RESET_DAY="$input_reset_day"
    fi

    read -p "是否需要校准流量统计? (y/N): " need_calibration
    need_calibration_lower=$(echo "$need_calibration" | tr '[:upper:]' '[:lower:]')
    if [[ "$need_calibration_lower" == "y" || "$need_calibration_lower" == "yes" ]]; then
        read -p "上传流量校准值 (字节，默认: 0): " input_cal_tx
        read -p "下载流量校准值 (字节，默认: 0): " input_cal_rx
        if [ -n "$input_cal_tx" ]; then
            NETWORK_CALIBRATION_TX="$input_cal_tx"
        fi
        if [ -n "$input_cal_rx" ]; then
            NETWORK_CALIBRATION_RX="$input_cal_rx"
        fi
        log_info "流量校准已配置: TX=$NETWORK_CALIBRATION_TX, RX=$NETWORK_CALIBRATION_RX"
    fi

    # 验证输入
    if [ -z "$HTTP_SERVER" ] || [ -z "$TOKEN" ]; then
        log_error "Http 地址和 Token 不能为空。"
        exit 1
    fi

    log_info "配置信息确认:"
    echo "  - Http Server: $HTTP_SERVER"
    echo "  - Token: ********"
    echo "  - 虚假倍率: $FAKE"
    echo "  - 上传间隔: $INTERVAL ms"
    echo "  - 启用 TLS: ${TLS_FLAG:--}"
    echo "  - 忽略证书: ${IGNORE_CERT_FLAG:--}"
    echo "  - 启用 Terminal: ${TERMINAL_FLAG:--}"
    echo "  - 流量重置日: 每月 $NETWORK_RESET_DAY 号"
    echo "  - 流量校准 TX: $NETWORK_CALIBRATION_TX 字节"
    echo "  - 流量校准 RX: $NETWORK_CALIBRATION_RX 字节"
    echo ""

    # --- 安装流程 ---
    install_dependencies

    ARCH_FILE=$(get_arch)
    DOWNLOAD_URL="https://ghfast.top/https://github.com/${GITHUB_REPO}/releases/download/latest/${ARCH_FILE}"

    log_info "检测到系统架构: $(uname -m)"
    log_info "准备从以下地址下载文件: ${DOWNLOAD_URL}"

    if ! wget -O "${INSTALL_PATH}" "${DOWNLOAD_URL}"; then
        log_error "下载失败！请检查网络连接或确认该架构的文件是否存在。"
        exit 1
    fi

    chmod +x "${INSTALL_PATH}"
    log_info "程序已成功下载并安装到: ${INSTALL_PATH}"

    # --- 创建网络配置文件 ---
    log_info "正在创建网络配置文件: ${NETWORK_CONFIG_FILE}"
    cat > ${NETWORK_CONFIG_FILE} <<EOF
# Komari Monitor Network Configuration
# 此文件支持热重载，修改后程序会自动检测并应用

# 禁用网络统计 (true/false)
disable_network_statistics=${DISABLE_NETWORK_STATS}

# 每月流量重置日 (1-31)
network_reset_day=${NETWORK_RESET_DAY}

# 采样间隔（秒）
network_interval=60

# 写入磁盘间隔（采样次数）
network_interval_number=10

# 流量校准值（字节）
# 用于与 VPS 服务商统计对齐
network_calibration_tx=${NETWORK_CALIBRATION_TX}
network_calibration_rx=${NETWORK_CALIBRATION_RX}
EOF

    chmod 644 ${NETWORK_CONFIG_FILE}
    log_info "网络配置文件已创建，您可以随时编辑此文件来调整校准值"

    # --- 创建 systemd 服务 ---
    log_info "正在创建 systemd 服务..."

    # 构建启动命令
    EXEC_START_CMD="${INSTALL_PATH} --http-server \"${HTTP_SERVER}\" --token \"${TOKEN}\" --fake \"${FAKE}\" --realtime-info-interval \"${INTERVAL}\""
    if [ -n "$TLS_FLAG" ]; then
        EXEC_START_CMD="$EXEC_START_CMD $TLS_FLAG"
    fi
    if [ -n "$IGNORE_CERT_FLAG" ]; then
        EXEC_START_CMD="$EXEC_START_CMD $IGNORE_CERT_FLAG"
    fi
    if [ -n "$TERMINAL_FLAG" ]; then
        EXEC_START_CMD="$EXEC_START_CMD $TERMINAL_FLAG"
    fi
    # 添加网络配置参数
    EXEC_START_CMD="$EXEC_START_CMD --config-path \"${NETWORK_CONFIG_FILE}\""
    EXEC_START_CMD="$EXEC_START_CMD --network-reset-day \"${NETWORK_RESET_DAY}\""
    EXEC_START_CMD="$EXEC_START_CMD --network-calibration-tx \"${NETWORK_CALIBRATION_TX}\""
    EXEC_START_CMD="$EXEC_START_CMD --network-calibration-rx \"${NETWORK_CALIBRATION_RX}\""
    if [ "$DISABLE_NETWORK_STATS" = "true" ]; then
        EXEC_START_CMD="$EXEC_START_CMD --disable-network-statistics"
    fi

    # 使用 cat 和 EOF 创建服务文件
    cat > ${SERVICE_FILE} <<EOF
[Unit]
Description=Komari Monitor RS Service
After=network.target

[Service]
Type=simple
User=root
ExecStart=${EXEC_START_CMD}
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

    log_info "systemd 服务文件已创建: ${SERVICE_FILE}"

    # --- 启用并启动服务 ---
    log_info "正在重载 systemd, 启用并启动服务..."
    systemctl daemon-reload
    systemctl enable ${SERVICE_NAME}
    systemctl restart ${SERVICE_NAME} # 使用 restart 确保服务是最新的

    # --- 检查服务状态 ---
    sleep 2 # 等待服务启动
    if systemctl is-active --quiet ${SERVICE_NAME}; then
        log_info "服务 '${SERVICE_NAME}' 已成功启动并正在运行！"
        log_info "您可以使用 'systemctl status ${SERVICE_NAME}' 命令查看服务状态。"
        log_info "您可以使用 'journalctl -u ${SERVICE_NAME} -f' 命令查看实时日志。"
        echo ""
        log_info "=== 网络流量配置说明 ==="
        log_info "配置文件位置: ${NETWORK_CONFIG_FILE}"
        log_info "您可以随时编辑此文件来调整流量校准值，程序会自动检测并应用更改"
        log_info "示例: 如果服务商统计的流量比程序多 1GB，可设置 network_calibration_tx=1073741824"
        log_info "每月 ${NETWORK_RESET_DAY} 号会自动重置流量统计"
    else
        log_error "服务 '${SERVICE_NAME}' 启动失败！"
        log_error "请使用 'systemctl status ${SERVICE_NAME}' 和 'journalctl -u ${SERVICE_NAME}' 命令检查错误详情。"
        exit 1
    fi
}

# --- 执行主程序 ---
main "$@"