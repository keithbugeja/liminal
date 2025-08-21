#!/bin/bash

# LED Control Script for ESP32 Liminal Firmware
# Usage: ./led_control.sh [command] [parameters...]
# 
# Commands:
#   on                    - Turn LED on
#   off                   - Turn LED off
#   toggle                - Toggle LED state
#   brightness <0-255>    - Set brightness (PWM capable pins only)
#   blink <on_ms> <off_ms> [cycles] - Start blinking pattern
#   stop_blink            - Stop blinking
#   status                - Get LED status
#   interactive           - Interactive mode with menu

BROKER="localhost"
DEVICE_ID="esp32-001"
LED_NAME="status_led"

TOPIC_BASE="liminal"
TOPIC_COMMANDS="$TOPIC_BASE/commands/$DEVICE_ID"
TOPIC_LED="$TOPIC_COMMANDS/devices/$LED_NAME"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_success() { echo -e "${GREEN}✓${NC} $1"; }
print_error() { echo -e "${RED}✗${NC} $1"; }
print_info() { echo -e "${BLUE}ℹ${NC} $1"; }
print_warning() { echo -e "${YELLOW}⚠${NC} $1"; }

# Function to check dependencies
check_dependencies() {
    if ! command -v mosquitto_pub &> /dev/null; then
        print_error "'mosquitto_pub' command not found. Please install mosquitto-clients."
        echo "On macOS: brew install mosquitto"
        echo "On Ubuntu: sudo apt-get install mosquitto-clients"
        exit 1
    fi
}

# Function to publish MQTT command
publish_command() {
    local message="$1"
    local description="$2"
    
    print_info "Sending: $description"
    echo "Topic: $TOPIC_LED"
    echo "Message: $message"
    
    mosquitto_pub -h $BROKER -t $TOPIC_LED -m "$message"
    
    if [ $? -eq 0 ]; then
        print_success "Command sent successfully"
    else
        print_error "Failed to send command. Is the MQTT broker running?"
        return 1
    fi
}

# Command functions
cmd_on() {
    local message='{"state": true}'
    publish_command "$message" "Turn LED ON"
}

cmd_off() {
    local message='{"state": false}'
    publish_command "$message" "Turn LED OFF"
}

cmd_toggle() {
    local message='{"toggle": true}'
    publish_command "$message" "Toggle LED state"
}

cmd_brightness() {
    local brightness="$1"
    
    if [[ -z "$brightness" ]]; then
        print_error "Brightness value required (0-255)"
        return 1
    fi
    
    if ! [[ "$brightness" =~ ^[0-9]+$ ]] || [ "$brightness" -lt 0 ] || [ "$brightness" -gt 255 ]; then
        print_error "Brightness must be a number between 0 and 255"
        return 1
    fi
    
    local message="{\"brightness\": $brightness}"
    publish_command "$message" "Set brightness to $brightness"
}

cmd_blink() {
    local on_time="$1"
    local off_time="$2"
    local cycles="$3"
    
    if [[ -z "$on_time" ]] || [[ -z "$off_time" ]]; then
        print_error "Usage: blink <on_time_ms> <off_time_ms> [cycles]"
        print_info "Examples:"
        print_info "  blink 500 500      - Blink every 500ms indefinitely"
        print_info "  blink 200 800 5    - Fast on, slow off, 5 times"
        print_info "  blink 1000 1000 3  - Slow blink, 3 times"
        return 1
    fi
    
    if ! [[ "$on_time" =~ ^[0-9]+$ ]] || ! [[ "$off_time" =~ ^[0-9]+$ ]]; then
        print_error "On time and off time must be positive numbers"
        return 1
    fi
    
    local message
    if [[ -n "$cycles" ]]; then
        if ! [[ "$cycles" =~ ^[0-9]+$ ]]; then
            print_error "Cycles must be a positive number"
            return 1
        fi
        message="{\"blink\": {\"on_time\": $on_time, \"off_time\": $off_time, \"cycles\": $cycles}}"
        publish_command "$message" "Start blinking: ${on_time}ms on, ${off_time}ms off, $cycles cycles"
    else
        message="{\"blink\": {\"on_time\": $on_time, \"off_time\": $off_time}}"
        publish_command "$message" "Start blinking: ${on_time}ms on, ${off_time}ms off, infinite"
    fi
}

cmd_stop_blink() {
    local message='{"stop_blink": true}'
    publish_command "$message" "Stop blinking"
}

cmd_status() {
    print_info "LED status commands are not implemented in this script"
    print_info "Monitor the device serial output or MQTT status topics for LED status"
    print_info "Status topic: liminal/status/$DEVICE_ID"
}

# Interactive mode
cmd_interactive() {
    print_info "LED Control - Interactive Mode"
    print_info "Broker: $BROKER"
    print_info "Topic: $TOPIC_LED"
    echo
    
    while true; do
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo "LED Control Options:"
        echo "  1) Turn ON"
        echo "  2) Turn OFF" 
        echo "  3) Toggle"
        echo "  4) Set Brightness"
        echo "  5) Start Blinking"
        echo "  6) Stop Blinking"
        echo "  7) Quick Patterns"
        echo "  q) Quit"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        
        read -p "Choose option: " choice
        
        case $choice in
            1)
                cmd_on
                ;;
            2)
                cmd_off
                ;;
            3)
                cmd_toggle
                ;;
            4)
                read -p "Enter brightness (0-255): " brightness
                cmd_brightness "$brightness"
                ;;
            5)
                read -p "Enter ON time (ms): " on_time
                read -p "Enter OFF time (ms): " off_time
                read -p "Enter cycles (empty for infinite): " cycles
                cmd_blink "$on_time" "$off_time" "$cycles"
                ;;
            6)
                cmd_stop_blink
                ;;
            7)
                echo "Quick Patterns:"
                echo "  a) Fast blink (200ms on/off)"
                echo "  b) Slow blink (1000ms on/off)" 
                echo "  c) Heartbeat (100ms on, 900ms off)"
                echo "  d) SOS pattern"
                read -p "Choose pattern: " pattern
                
                case $pattern in
                    a)
                        cmd_blink 200 200
                        ;;
                    b)
                        cmd_blink 1000 1000
                        ;;
                    c)
                        cmd_blink 100 900
                        ;;
                    d)
                        print_info "SOS Pattern: ... --- ..."
                        # Short blinks (S)
                        cmd_blink 200 200 3
                        sleep 2
                        # Long blinks (O)  
                        cmd_blink 600 200 3
                        sleep 2
                        # Short blinks (S)
                        cmd_blink 200 200 3
                        ;;
                    *)
                        print_warning "Invalid pattern choice"
                        ;;
                esac
                ;;
            q|Q)
                print_info "Goodbye!"
                break
                ;;
            *)
                print_warning "Invalid choice. Please try again."
                ;;
        esac
        
        echo
        read -p "Press Enter to continue..."
        echo
    done
}

# Show usage
show_usage() {
    echo "LED Control Script for ESP32 Liminal Firmware"
    echo
    echo "Usage: $0 [command] [parameters...]"
    echo
    echo "Commands:"
    echo "  on                              - Turn LED on"
    echo "  off                             - Turn LED off"
    echo "  toggle                          - Toggle LED state"
    echo "  brightness <0-255>              - Set brightness (PWM capable pins only)"
    echo "  blink <on_ms> <off_ms> [cycles] - Start blinking pattern"
    echo "  stop_blink                      - Stop blinking"
    echo "  status                          - Get LED status info"
    echo "  interactive                     - Interactive mode with menu"
    echo
    echo "Examples:"
    echo "  $0 on                           # Turn LED on"
    echo "  $0 brightness 128               # Set to 50% brightness"
    echo "  $0 blink 500 500                # Blink every 500ms indefinitely"
    echo "  $0 blink 200 800 5              # Fast on, slow off, 5 times"
    echo "  $0 interactive                  # Start interactive mode"
    echo
    echo "Configuration:"
    echo "  Broker: $BROKER"
    echo "  Device: $DEVICE_ID"
    echo "  LED: $LED_NAME"
    echo "  Topic: $TOPIC_LED"
}

# Main script
main() {
    check_dependencies
    
    if [ $# -eq 0 ]; then
        show_usage
        exit 0
    fi
    
    local command="$1"
    shift
    
    case "$command" in
        on)
            cmd_on
            ;;
        off)
            cmd_off
            ;;
        toggle)
            cmd_toggle
            ;;
        brightness)
            cmd_brightness "$1"
            ;;
        blink)
            cmd_blink "$1" "$2" "$3"
            ;;
        stop_blink)
            cmd_stop_blink
            ;;
        status)
            cmd_status
            ;;
        interactive|menu)
            cmd_interactive
            ;;
        help|--help|-h)
            show_usage
            ;;
        *)
            print_error "Unknown command: $command"
            echo
            show_usage
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
