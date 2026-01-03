#!/bin/bash
# Komari Monitor - Traffic Statistics Testing Script
# This script helps test traffic statistics and reset functionality

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
RUNTIME_DATA_PATH="/var/lib/komari-monitor/network-data.conf"
if [ "$EUID" -ne 0 ]; then
    RUNTIME_DATA_PATH="$HOME/.local/share/komari-monitor/network-data.conf"
fi

BOOT_ID_PATH="/proc/sys/kernel/random/boot_id"

# Helper functions
print_header() {
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}\n"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

# Read current runtime data
read_runtime_data() {
    if [ -f "$RUNTIME_DATA_PATH" ]; then
        echo -e "\n${GREEN}Current Runtime Data:${NC}"
        cat "$RUNTIME_DATA_PATH"
        echo ""
    else
        print_warning "Runtime data file not found: $RUNTIME_DATA_PATH"
    fi
}

# Backup runtime data
backup_runtime_data() {
    if [ -f "$RUNTIME_DATA_PATH" ]; then
        cp "$RUNTIME_DATA_PATH" "${RUNTIME_DATA_PATH}.backup"
        print_success "Backed up runtime data to ${RUNTIME_DATA_PATH}.backup"
    fi
}

# Restore runtime data
restore_runtime_data() {
    if [ -f "${RUNTIME_DATA_PATH}.backup" ]; then
        cp "${RUNTIME_DATA_PATH}.backup" "$RUNTIME_DATA_PATH"
        print_success "Restored runtime data from backup"
        rm "${RUNTIME_DATA_PATH}.backup"
    fi
}

# Test 1: View current runtime data
test_view_data() {
    print_header "Test 1: View Current Runtime Data"
    read_runtime_data
}

# Test 2: Simulate system reboot
test_simulate_reboot() {
    print_header "Test 2: Simulate System Reboot"

    if [ ! -f "$RUNTIME_DATA_PATH" ]; then
        print_error "Runtime data file not found. Please run the program first."
        return 1
    fi

    print_info "Current runtime data:"
    read_runtime_data

    backup_runtime_data

    # Get current boot_id
    if [ -f "$BOOT_ID_PATH" ]; then
        CURRENT_BOOT_ID=$(cat "$BOOT_ID_PATH" | tr -d '[:space:]')
        print_info "Current boot_id: $CURRENT_BOOT_ID"
    else
        print_error "Cannot read boot_id (not on Linux?)"
        return 1
    fi

    # Modify boot_id in runtime data to simulate reboot
    FAKE_BOOT_ID="00000000-0000-0000-0000-000000000000"
    sed -i "s/^boot_id=.*/boot_id=$FAKE_BOOT_ID/" "$RUNTIME_DATA_PATH"

    print_success "Modified boot_id to simulate reboot"
    print_info "Modified runtime data:"
    read_runtime_data

    print_warning "Now restart the komari-monitor program to test reboot behavior"
    print_info "The program should:"
    echo "  1. Detect boot_id change"
    echo "  2. Merge current_boot_tx/rx into accumulated_tx/rx"
    echo "  3. Reset boot_source_tx/rx to current system traffic"
    echo "  4. Reset current_boot_tx/rx to 0"
    echo "  5. Update boot_id to current value"

    echo -e "\n${YELLOW}After testing, run: $0 restore${NC}"
}

# Test 3: Simulate monthly reset
test_simulate_monthly_reset() {
    print_header "Test 3: Simulate Monthly Reset"

    if [ ! -f "$RUNTIME_DATA_PATH" ]; then
        print_error "Runtime data file not found. Please run the program first."
        return 1
    fi

    print_info "Current runtime data:"
    read_runtime_data

    backup_runtime_data

    # Get current month
    CURRENT_MONTH=$(date +%-m)
    PREVIOUS_MONTH=$((CURRENT_MONTH - 1))
    if [ $PREVIOUS_MONTH -eq 0 ]; then
        PREVIOUS_MONTH=12
    fi

    print_info "Current month: $CURRENT_MONTH"
    print_info "Setting last_reset_month to: $PREVIOUS_MONTH"

    # Modify last_reset_month to simulate month change
    sed -i "s/^last_reset_month=.*/last_reset_month=$PREVIOUS_MONTH/" "$RUNTIME_DATA_PATH"

    print_success "Modified last_reset_month to simulate month change"
    print_info "Modified runtime data:"
    read_runtime_data

    print_warning "Now restart the komari-monitor program to test monthly reset"
    print_info "The program should:"
    echo "  1. Detect month change (should_reset_traffic returns true)"
    echo "  2. Reset accumulated_tx/rx to 0"
    echo "  3. Reset current_boot_tx/rx to 0"
    echo "  4. Set boot_source_tx/rx to current system traffic (new baseline)"
    echo "  5. Update last_reset_month to current month"

    echo -e "\n${YELLOW}After testing, run: $0 restore${NC}"
}

# Test 4: Test edge cases (month-end scenarios)
test_month_end_scenarios() {
    print_header "Test 4: Month-End Scenarios"

    print_info "This test verifies month-end handling logic"
    echo ""

    echo "Scenario 1: reset_day=31, current month=February"
    echo "  Expected: effective_reset_day should be 28 (or 29 in leap year)"
    echo ""

    echo "Scenario 2: reset_day=31, current month=April"
    echo "  Expected: effective_reset_day should be 30"
    echo ""

    echo "Scenario 3: reset_day=15, any month"
    echo "  Expected: effective_reset_day should be 15"
    echo ""

    print_info "You can test by setting reset_day=31 in config and checking logs"
    print_info "Log messages will show: 'configured day: 31, effective day: XX'"
}

# Test 5: Test combined scenario (reboot + monthly reset)
test_combined_scenario() {
    print_header "Test 5: Combined Scenario (Reboot + Monthly Reset)"

    if [ ! -f "$RUNTIME_DATA_PATH" ]; then
        print_error "Runtime data file not found. Please run the program first."
        return 1
    fi

    print_info "Current runtime data:"
    read_runtime_data

    backup_runtime_data

    # Modify both boot_id and last_reset_month
    FAKE_BOOT_ID="00000000-0000-0000-0000-000000000000"
    CURRENT_MONTH=$(date +%-m)
    PREVIOUS_MONTH=$((CURRENT_MONTH - 1))
    if [ $PREVIOUS_MONTH -eq 0 ]; then
        PREVIOUS_MONTH=12
    fi

    sed -i "s/^boot_id=.*/boot_id=$FAKE_BOOT_ID/" "$RUNTIME_DATA_PATH"
    sed -i "s/^last_reset_month=.*/last_reset_month=$PREVIOUS_MONTH/" "$RUNTIME_DATA_PATH"

    print_success "Modified both boot_id and last_reset_month"
    print_info "Modified runtime data:"
    read_runtime_data

    print_warning "Now restart the komari-monitor program"
    print_info "Expected behavior:"
    echo "  1. First: Handle reboot (merge current_boot into accumulated)"
    echo "  2. Then: Detect monthly reset and reset all counters"
    echo "  3. Result: All traffic should be reset to 0 (monthly reset takes priority)"

    echo -e "\n${YELLOW}After testing, run: $0 restore${NC}"
}

# Test 6: Verify traffic accumulation
test_traffic_accumulation() {
    print_header "Test 6: Traffic Accumulation Verification"

    print_info "This test helps verify traffic is accumulating correctly"
    echo ""

    if [ -f "$RUNTIME_DATA_PATH" ]; then
        echo "Step 1: Record initial values"
        read_runtime_data | grep -E "(boot_source|current_boot|accumulated)"

        echo ""
        print_info "Step 2: Generate some traffic (download a file, browse web, etc.)"
        print_info "Step 3: Wait for 10+ seconds (default save interval)"
        print_info "Step 4: Check runtime data again"
        echo ""
        echo "Expected changes:"
        echo "  - current_boot_tx/rx should increase"
        echo "  - boot_source_tx/rx should remain unchanged"
        echo "  - accumulated_tx/rx should remain unchanged (until reboot)"
        echo ""
        echo "Total traffic shown = accumulated + current_boot + calibration"
    else
        print_error "Runtime data file not found"
    fi
}

# Show usage
show_usage() {
    cat << EOF
${GREEN}Komari Monitor - Traffic Statistics Testing Script${NC}

Usage: $0 <command>

Commands:
  view              View current runtime data
  reboot            Simulate system reboot
  monthly           Simulate monthly reset
  month-end         Information about month-end scenarios
  combined          Simulate reboot + monthly reset combined
  accumulation      Verify traffic accumulation
  restore           Restore backed up runtime data
  help              Show this help message

Examples:
  $0 view                 # View current traffic data
  $0 reboot               # Test system reboot scenario
  $0 monthly              # Test monthly reset
  $0 restore              # Restore backup after testing

${YELLOW}Important Notes:${NC}
  - Always backup your data before testing
  - The script automatically creates backups for destructive tests
  - Remember to restore after testing: $0 restore
  - Check program logs for detailed behavior during tests

${BLUE}Test Workflow:${NC}
  1. Run test command (reboot/monthly/combined)
  2. Restart komari-monitor program
  3. Check logs to verify expected behavior
  4. Restore backup data
EOF
}

# Main script logic
case "${1:-help}" in
    view)
        test_view_data
        ;;
    reboot)
        test_simulate_reboot
        ;;
    monthly)
        test_simulate_monthly_reset
        ;;
    month-end)
        test_month_end_scenarios
        ;;
    combined)
        test_combined_scenario
        ;;
    accumulation)
        test_traffic_accumulation
        ;;
    restore)
        restore_runtime_data
        ;;
    help|*)
        show_usage
        ;;
esac
