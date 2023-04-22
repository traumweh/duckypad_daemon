#!/bin/sh
window=$(swaymsg -t get_tree | jq '.. | select(.type?) | select(.focused==true)')

if [ "$window" == "" ]; then
  echo '{"title":"","process_name":"","process_id":0}'
else
  title=$(echo "$window" | jq '.name')
  process_name=$(echo "$window" | jq '.app_id')
  process_id=$(echo "$window" | jq '.pid')
  echo '{"title":'$title',"process_name":'$process_name',"process_id":'$process_id'}'
fi
