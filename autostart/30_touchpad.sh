#!/usr/bin/env bash

printf '%s' "configuring synclient" >>"$LOGFILE"
synclient VertEdgeScroll=0
synclient VertTwoFingerScroll=1
synclient MaxSpeed=2.2
synclient AccelFactor=0.08
synclient TapButton1=1
synclient CoastingSpeed=0
synclient PalmDetect=1
synclient PalmMinWidth=20
synclient PalmMinZ=180
