#!/bin/bash
# Convert IP to network byte order hex (inet_addr format)
IP=$1
IFS=. read -r a b c d <<< "$IP"
printf "0x%02x%02x%02x%02x" "$a" "$b" "$c" "$d"
