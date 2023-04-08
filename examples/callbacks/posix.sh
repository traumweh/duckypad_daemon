#!/bin/sh
profile=
title=
process_name=

while getopts p:t:n: name
do
  case $name in
  p)  profile="$OPTARG";;
  t)  title="$OPTARG";;
  n)  process_name="$OPTARG";;
  ?)  printf "Usage: %s: [-p profile] [-t title] [-n process_name]\n" $0
      exit 2;;
  esac
done

echo "Got the following values:"
echo "Profile: $profile"
echo "Title: $title"
echo "Process Name: $process_name"
