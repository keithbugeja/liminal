#!/bin/bash
BROKER="localhost"
TOPIC="test/topic"

for i in {1..10}
do
    MESSAGE="{\"message\": \"hello world $i\"}"
    echo "Publishing: $MESSAGE"
    mosquitto_pub -h $BROKER -t $TOPIC -m "$MESSAGE"
    sleep 1
done

TOPIC="test/acceleration"
for i in {1..10}
do
    MESSAGE="{\"acceleration\": $((i * 10))}"
    echo "Publishing: $MESSAGE"
    mosquitto_pub -h $BROKER -t $TOPIC -m "$MESSAGE"
    sleep 1
done