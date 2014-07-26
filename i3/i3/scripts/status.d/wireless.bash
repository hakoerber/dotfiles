#!/bin/bash
INTERFACE="wlp2s0"
TIMEFORMAT="+%F %R"

iwconfig_output="$(iwconfig $INTERFACE)"

columnize() {
    column -t --separator="|"
}

essid=$(echo "$iwconfig_output" | sed -n "1p" | awk -F '"' '{print $2}')
mode=$(echo "$iwconfig_output" | sed -n "1p" | awk -F " " '{print $3}')
freq=$(echo "$iwconfig_output" | sed -n "2p" | awk -F " " '{print $2}' | cut -d":" -f2)
mac=$(echo "$iwconfig_output" | sed -n "2p" | awk -F " " '{print $6}')
qual=$(echo "$iwconfig_output" | sed -n "6p" | awk -F " " '{print $2}' | cut -d"=" -f2)
lvl=$(echo "$iwconfig_output" | sed -n "6p" | awk -F " " '{print $4}' | cut -d"=" -f2)
rate=$(echo "$iwconfig_output" | sed -n "3p" | awk -F "=" '{print $2}' | cut -d" " -f1)

ip=$(ip addr show $INTERFACE | grep "[[:space:]]*inet" | head -n 1 | tr -s " " | cut -d " " -f 3 | cut -d "/" -f 1)

vnstat_output="$(vnstat --dumpdb)"
txhour=$(echo "$vnstat_output" | grep "^h;0;" | cut -d ";" -f 5)
rxhour=$(echo "$vnstat_output" | grep "^h;0;" | cut -d ";" -f 4)
txtoday=$(echo "$vnstat_output" | grep "^d;0;" | cut -d ";" -f 5)
rxtoday=$(echo "$vnstat_output" | grep "^d;0;" | cut -d ";" -f 4)
txtotal=$(echo "$vnstat_output" | grep "^totaltx;" | cut -d ";" -f 2)
rxtotal=$(echo "$vnstat_output" | grep "^totalrx;" | cut -d ";" -f 2)
vnstat_created="$(date --date=@$(echo "$vnstat_output" | grep "^created;" | cut -d ";" -f 2) "$TIMEFORMAT")"
vnstat_last_update="$(date --date=@$(echo "$vnstat_output" | grep "^updated;" | cut -d ";" -f 2) "$TIMEFORMAT")"

txhour=$(( $txhour / 1024 ))
rxhour=$(( $rxhour / 1024 ))

echo -e "Interface $INTERFACE:\n"
(
echo "IP:|$ip"
echo "ESSID:|$essid"
echo "Mode:|$mode"
echo "Frequency:|$freq"
echo "MAC address:|$mac"
echo "Quality:|$qual"
echo "Signal level:|$lvl dBm"
echo "Bitrate:|$rate Mb/s"
) | columnize
echo -e "\nUsage:\n"
(
echo "Hourly up:|${txhour}|MiB"
echo "Hourly down:|${rxhour}|MiB"
echo "Daily up:|${txtoday}|MiB"
echo "Daily down:|${rxtoday}|MiB"
echo "Total up:|${txtotal}|MiB"
echo "Total down:|${rxtotal}|MiB"
echo ""
) | columnize
echo -e "\nLast update at: $vnstat_last_update"
