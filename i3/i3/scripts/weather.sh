#!/bin/sh

#AccuWeather (r) RSS weather tool for conky
#
#USAGE: weather.sh <locationcode>
#
#(c) Michael Seiler 2007
    
METRIC=1 #Should be 0 or 1; 0 for F, 1 for C
    
if [ -z $1 ]; then
    echo
    echo "USAGE: weather.sh <locationcode>"
    echo
    exit 0;
fi
    
curl -s http://rss.accuweather.com/rss/liveweather_rss.asp\?metric\=${METRIC}\&locCode\=$1 | grep "Currently" | head -1 | grep -o 'Currently:[^<]*' | cut -d ' ' -f 2- | tr ':' ','
# | perl -ne 'if (/Currently/) {chomp;/\<title\>Currently: (.*)?\<\/title\>/; print "$1"; }' |sed s/\:// |tr [A-Z] [a-z]


