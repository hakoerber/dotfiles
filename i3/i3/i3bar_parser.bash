#!/usr/bin/env bash

while read line; do
    echo $line
    echo $line >> ~/.i3/output_i3bar.log
done
