#!/bin/sh
profile=
app_name=
title=
process_name=

while getopts p:a:t:n: name
do
  case $name in
  p)  profile="$OPTARG";;
  a)  app_name="$OPTARG";;
  t)  title="$OPTARG";;
  n)  process_name="$OPTARG";;
  ?)  printf "Usage: %s: [-p profile] [-a app_name] [-t title] [-n process_name]\n" $0
      exit 2;;
  esac
done

echo "Got the following values:"
echo "Profile: $profile"
echo "App Name: $app_name"
echo "Title: $title"
echo "Process Name: $process_name"
