#!/usr/bin/env python3
import argparse

parser = argparse.ArgumentParser()
parser.add_argument("-p", type=int, help="new profile")
parser.add_argument("-t", type=str, help="title of active window")
parser.add_argument("-n", type=str, help="process name of active window")
args = vars(parser.parse_args())

print("Got the following values:")
print("Profile: {}".format(args["p"]))
print("Title: {}".format(args["t"]))
print("Process Name: {}".format(args["n"]))
