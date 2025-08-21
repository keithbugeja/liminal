#!/bin/bash

# MPU6500 Accelerometer Data Generator
# Usage: ./message_gen_mpu6500.sh [iterations]
# If no iterations specified, runs infinitely

BROKER="localhost"
DEVICE_ID="esp32-001"
SENSOR_TYPE="imu"
IMU_TYPE="MPU6500"

TOPIC_BASE="liminal"
TOPIC_SENSORS="$TOPIC_BASE/sensors/$DEVICE_ID"
# TOPIC_COMMANDS="$TOPIC_BASE/commands/$DEVICE_ID"
# TOPIC_STATUS="$TOPIC_BASE/status/$DEVICE_ID"
TOPIC="$TOPIC_SENSORS/$SENSOR_TYPE"

# Parse command line arguments
ITERATIONS=${1:-"infinite"}

echo "Starting MPU6500 data generation..."
echo "Broker: $BROKER"
echo "Topic: $TOPIC"
echo "Iterations: $ITERATIONS"
echo "Press Ctrl+C to stop"
echo

# Function to generate random float between min and max
random_float() {
    local min=$1
    local max=$2
    local scale=$3
    echo "scale=$scale; $min + ($RANDOM / 32767) * ($max - $min)" | bc -l
}

# Function to generate and publish one message
generate_message() {
    local iteration=$1
    
    # Get current timestamp in milliseconds
    local timestamp=$(date +%s%3N)
    
    # Generate realistic accelerometer values
    # X and Y: -2.0 to 2.0 g (typical for device movement)
    # Z: 8.5 to 10.5 g (gravity + small variations)
    local accel_x=$(random_float -2.0 2.0 3)
    local accel_y=$(random_float -2.0 2.0 3)
    local accel_z=$(random_float 8.5 10.5 3)

    local gyro_x=$(random_float -2.0 2.0 3)
    local gyro_y=$(random_float -2.0 2.0 3)
    local gyro_z=$(random_float 8.5 10.5 3)

    local temp_v=$(random_float 20.0 45.0 1) 

    # Create JSON message
    local message=$(cat <<EOF
{
  "sensor_name":"main_imu",
  "sensor_type":"imu",
  "imu_type":"$IMU_TYPE",
  "timestamp": $timestamp,
  "device_id": "$DEVICE_ID",
  "accelerometer": {
    "x": $accel_x,
    "y": $accel_y,
    "z": $accel_z,
    "unit": "g"
  },
  "gyroscope": {
    "x": $gyro_x,
    "y": $gyro_y,
    "z": $gyro_z,
    "unit":"°/s"
  },
  "temperature": $temp_v,
  "temperature_unit":"°C"  
}
EOF
)
    
    # Remove newlines for single-line JSON
    local compact_message=$(echo "$message" | tr -d '\n' | tr -s ' ')
    
    echo "[$iteration] Publishing: $compact_message"
    mosquitto_pub -h $BROKER -t $TOPIC -m "$compact_message"
    
    if [ $? -ne 0 ]; then
        echo "Error: Failed to publish message. Is mosquitto_pub installed and broker running?"
        exit 1
    fi
}

# Check if bc is available (for floating point math)
if ! command -v bc &> /dev/null; then
    echo "Error: 'bc' command not found. Please install bc for floating point calculations."
    echo "On macOS: brew install bc"
    echo "On Ubuntu: sudo apt-get install bc"
    exit 1
fi

# Check if mosquitto_pub is available
if ! command -v mosquitto_pub &> /dev/null; then
    echo "Error: 'mosquitto_pub' command not found. Please install mosquitto-clients."
    echo "On macOS: brew install mosquitto"
    echo "On Ubuntu: sudo apt-get install mosquitto-clients"
    exit 1
fi

# Main loop
if [ "$ITERATIONS" = "infinite" ]; then
    echo "Running infinitely (Ctrl+C to stop)..."
    counter=1
    while true; do
        generate_message $counter
        sleep 1
        ((counter++))
    done
else
    # Validate iterations is a number
    if ! [[ "$ITERATIONS" =~ ^[0-9]+$ ]]; then
        echo "Error: Iterations must be a positive number or omitted for infinite"
        echo "Usage: $0 [iterations]"
        exit 1
    fi
    
    echo "Running for $ITERATIONS iterations..."
    for i in $(seq 1 $ITERATIONS); do
        generate_message $i
        sleep 1
    done
fi

echo "Data generation completed."
