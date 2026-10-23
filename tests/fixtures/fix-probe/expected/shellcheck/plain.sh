#!/bin/sh
x=ab; echo "$1"
cd /tmp || exit
y=$(date)
echo "$y $x"
