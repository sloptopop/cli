#!/bin/sh
x=ab; z="é😀"; echo "$1"
z="é😀"; cd /tmp || exit
y=$(echo é😀)
echo "$y $x"
