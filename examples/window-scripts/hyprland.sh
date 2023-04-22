#!/bin/sh
window=$(hyprctl activewindow)

if [ "$window" == "Invalid" ]; then
  echo '{"title":"","process_name":"","process_id":0}'
else
  title=$(echo "$window" | grep -P '^Window .*? -> ' | sed -r 's|^Window .*? -> (.*?):$|\1|')
  process_name=$(echo "$window" | grep -P '^\tclass: ' | sed -r 's|^\s*class: (.*)$|\1|' | sed -e 's|"|\"|g')
  process_id=$(echo "$window" | grep -P '^\tpid: ' | sed -r 's|^\s*pid: (.*)$|\1|')
  echo '{"title":"'$title'","process_name":"'$process_name'","process_id":'$process_id'}'
fi
