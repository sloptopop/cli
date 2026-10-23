#!/bin/sh
x=ab; z="é😀"; echo $1
z="é😀"; cd /tmp
y=`echo é😀`
echo "$y $x"
