set -o nounset
set -o errexit

host="${1}"
shift

cd /root

curl --proto '=https' --tlsv1.2 --fail --output cfg https://de-hkoerber-public.s3.eu-central-1.amazonaws.com/cfg
install --mode 700 cfg /usr/local/bin/cfg

/usr/local/bin/cfg --hostname "${host}" bootstrap --i-am-sure

read -rp "> Ready for reboot. Press enter for shutdown, then remove the installation media and boot again "

poweroff
