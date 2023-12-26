#!/bin/bash

if [ -z "$1" ]; then
   echo "Please specify the amount of runs"
   exit 1
fi
runs=$1

if [ -f "./target/release/odd" ]; then
   echo "Can't find binary"
   exit 1
fi

for ((i=1; i<="$runs"; i++)); do
    bash -c "time ./target/release/odd --boots 500 > /dev/null" &> >(grep user)
done | grep -Po "\d+(?=s)" > "results.txt"

sum=0

while IFS= read -r line; do
  number=$((10#$line))
  sum=$((sum + number))
done < "results.txt"

average=$(echo "scale=3; $sum / $runs" | bc)

echo "Average: $average"
