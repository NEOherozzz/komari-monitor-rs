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
    TERMINAL_FLAG=""

    while [ "$#" -gt 0 ]; do
        case "$1" in
            --http-server) HTTP_SERVER="$2"; shift 2;;
            -t|--token) TOKEN="$2"; shift 2;;
            -f|--fake) FAKE="$2"; shift 2;;
            --realtime-info-interval) INTERVAL="$2"; shift 2;;
            --tls) TLS_FLAG="--tls"; shift 1;;
            --ignore-unsafe-cert) IGNORE_CERT_FLAG="--ignore-unsafe-cert"; shift 1;;
            --terminal) TERMINAL_FLAG="--terminal"; shift 1;;
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

    if [ -z "$TERMINAL_FLAG" ]; then
        read -p "是否启用 Web Terminal 功能? (y/N): " enable_terminal
        enable_terminal_lower=$(echo "$enable_terminal" | tr '[:upper:]' '[:lower:]')
        if [[ "$enable_terminal_lower" == "y" || "$enable_terminal_lower" == "yes" ]]; then
            TERMINAL_FLAG="--terminal"
        fi
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
    echo "  启用 Terminal: ${TERMINAL_FLAG:--}"
    echo ""

    # 安装依赖
    install_dependencies

    # 下载程序
    ARCH_FILE=$(get_arch)
    DOWNLOAD_URL="https://ghfast.top/https://github.com/${GITHUB_REPO}/releases/download/latest/${ARCH_FILE}"

    log_info "检测到系统架构: $(uname -m)"
    log_info "正在下载: ${DOWNLOAD_URL}"

    if ! wget -O "${INSTALL_PATH}" "${DOWNLOAD_URL}"; then
        log_error "下载失败，请检查网络连接"
        exit 1
    fi

    chmod +x "${INSTALL_PATH}"
    log_success "程序已安装到: ${INSTALL_PATH}"

    # 创建配置文件
    cat > ${CONFIG_FILE} <<EOF
# Komari Monitor Agent Configuration
# Generated at $(date)

http_server=${HTTP_SERVER}
token=${TOKEN}
fake=${FAKE}
realtime_info_interval=${INTERVAL}
tls=${TLS_FLAG:+true}
ignore_unsafe_cert=${IGNORE_CERT_FLAG:+true}
terminal=${TERMINAL_FLAG:+true}
EOF

    log_success "配置文件已创建: ${CONFIG_FILE}"

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

    # 询问是否删除配置文件
    read -p "是否删除配置文件? (y/N): " delete_config
    delete_config_lower=$(echo "$delete_config" | tr '[:upper:]' '[:lower:]')

    if [[ "$delete_config_lower" == "y" || "$delete_config_lower" == "yes" ]]; then
        if [ -f "${CONFIG_FILE}" ]; then
            rm -f "${CONFIG_FILE}"
            log_success "已删除配置文件: ${CONFIG_FILE}"
        fi
    else
        log_info "保留配置文件: ${CONFIG_FILE}"
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
    cat <<EOF
${CYAN}Komari Agent 管理工具 v${VERSION}${NC}

${GREEN}使用方法:${NC}
  sudo kagent.sh <命令> [参数]

${GREEN}可用命令:${NC}
  ${YELLOW}install${NC}              安装 Komari Agent
                       参数: --http-server, --token, --terminal, 等
                       示例: sudo kagent.sh install --http-server http://example.com:8080 --token mytoken

  ${YELLOW}uninstall${NC}            卸载 Komari Agent

  ${YELLOW}start${NC}                启动服务
  ${YELLOW}stop${NC}                 停止服务
  ${YELLOW}restart${NC}              重启服务

  ${YELLOW}status${NC}               查看服务状态和配置信息

  ${YELLOW}logs${NC}                 查看实时日志

  ${YELLOW}config show${NC}          显示当前配置
  ${YELLOW}config edit${NC}          编辑配置文件
  ${YELLOW}config set${NC} <key> <value>
                       设置配置项
                       示例: sudo kagent.sh config set reset_day 5

  ${YELLOW}update${NC}               更新程序到最新版本

  ${YELLOW}help${NC}                 显示此帮助信息

${GREEN}常用配置项:${NC}
  http_server              HTTP 服务器地址
  token                    认证令牌
  terminal                 启用 Web Terminal (true/false)
  network_interval         网络统计采样间隔（秒）
  reset_day                流量重置日期（1-31）
  calibration_tx           上传流量校准值（字节）
  calibration_rx           下载流量校准值（字节）
  log_level                日志级别（error/warn/info/debug/trace）

${GREEN}文件位置:${NC}
  程序:     ${INSTALL_PATH}
  配置:     ${CONFIG_FILE}
  服务:     ${SERVICE_FILE}

${GREEN}更多信息:${NC}
  GitHub: https://github.com/${GITHUB_REPO}

EOF
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
