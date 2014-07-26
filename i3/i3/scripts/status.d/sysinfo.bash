columnate() {
    column -t --separator="|"
}

freeoutput="$(free -m)"
memtotal=$(echo "$freeoutput" | head -2 | tail -1 | tr -s " " | cut -d " " -f 2)
memused=$(echo "$freeoutput" | head -3 | tail -1 | tr -s " " | cut -d " " -f 3)


(echo "Current user:|$(whoami)"
echo "Hostname:|$(hostname)"
echo "Uptime:|$(uptime --pretty | cut -d " " -f 2-)"
echo "Kernel:|$(uname -r)"
echo "Packages:|$(pacman -Qq | wc -l)") | columnate
echo ""
echo "CPU:"
(echo "Name:|$(lscpu | grep "Model name:" | tr -s " " | cut -d " " -f 3-)"
echo "Architecture:|$(uname -m)"
echo "Load:|$(uptime | tr -s " " | cut -d " " -f 10 | tr -d ",")") | columnate
echo ""
echo "MEM:"
echo "${memused}MB / ${memtotal}MB ($(( $memused * 100 / $memtotal ))%) used"
echo ""
echo "PROCS:"
(echo "x x cpu% mem% x x x x x x command" ; ps aux | sort -nrk 3 | tr -s " " | cut -d " " -f -11 | uniq -uf 10 | head -10) | cut -d " " -f 3,4,11 |  column -t
echo ""
echo "STORAGE:"
df -hT --type=btrfs --type=ext4 --total
echo ""
echo "SWAP:"
if [[ -z "$(swapon)" ]] ; then
    echo "none"
else
    i=0
    swapon | (while read line; do
        [[ $i == 0 ]] && { i=1 ; continue ; }
        swappath=$(echo $line | cut -d ' ' -f 1)
        swapused=$(echo $line | cut -d ' ' -f 4)
        swaptotal=$(echo $line | cut -d ' ' -f 3)

        echo "${swappath}:|${swapused}/${swaptotal}"
    done) | columnate
fi

