_remote() {
    [[ -n "$SSH_CONNECTION" ]]
}

cd() {
    builtin cd "$@" && ls
}

mount() {
    if [[ $# == 0 ]] ; then
        command mount | column -t
    else
        command mount "$@"
    fi
}

extr()
{
    if [[ -f "$1" ]] ; then
        case "$1" in
            *.tar.bz2 ) tar xvjf "$1"     ;;
            *.tar.gz  ) tar xvzf "$1"     ;;
            *.tar.xz  ) tar xvJf "$1"     ;;
            *.bz2     ) bunzip2 "$1"      ;;
            *.rar     ) unrar x "$1"      ;;
            *.gz      ) gunzip "$1"       ;;
            *.tar     ) tar xvf "$1"      ;;
            *.tbz2    ) tar xvjf "$1"     ;;
            *.tgz     ) tar xvzf "$1"     ;;
            *.zip     ) unzip "$1"        ;;
            *.Z       ) uncompress "$1"   ;;
            *.7z      ) 7z x "$1"         ;;
            *)
                echo "$1 cannot be extracted via $0"
                ;;
        esac
    else
        echo "$1 is not a valid file"
    fi
}

ruler() {
    for s in '....^....|' '1234567890'; do
        w=${#s}
        str=$(for (( i=1; $i<=$(( ($COLUMNS + $w) / $w )) ; i=$i+1 )); do echo -n $s; done )
        str=$(echo $str | cut -c -$COLUMNS)
        echo $str
    done
}

addext() {
    [[ -z "$1" ]] || [[ -z "$2" ]] && { echo "Usage: $0 <file> <extension>" ; return }
    mv "$1" "$1$2"
}

rmext() {
    [[ -e "$1" ]] && mv -i "$1" "${1%.*}"
}


ckwww() {
    ping -c 3 www.google.com
}

httpcode() {
    curl http://httpcode.info/$1
}

bak() {
    if ! [[ "$1" ]] ; then
        printf '%s\n' "usage: $0 FILE"
        return 1
    fi
    if ! [[ -e "$1" ]] ; then
        printf '%s\n' "\"$1\" not found"
        return 1
    fi
    name="$1.$(date +%Y%m%d%H%M%S.bak)"
    if [[ -e "${name}" ]] ; then
        printf '%s\n' "Backup file \"$name\" already exists"
        return 1
    fi
    cp --archive --verbose --no-clobber "$1" "${name}"
}

fstab() {
    # yeah
    expand /etc/fstab | grep -v '^#' | grep -P '^.+$' | tr -s ' ' | tr ' ' '|' | cat <(grep -P '<.+>' /etc/fstab | cut -f 2- -d ' ' | sed 's/>[^<]*</>|</g') - | column -ts '|'
}

serve() {
    python3 -m http.server 8800
}

manpdf() {
    [[ -z "$1" ]] && { printf '%s' >&2 "$(man)" ; return ; }
    man -t "$1" | ps2pdf - - | zathura -
}

myip4() {
    curl "http://ipv4.icanhazip.com"
}

myip6() {
    curl "http://ipv6.icanhazip.com" 2>/dev/null || echo "no ip6"
}

diffdir() {
    [[ "$1" ]] && [[ "$2" ]] || { echo "$0 <dir1> <dir2>" ; return 1 ; }
    diff <(cd "$1" && find -type f -exec md5sum {} \;) <(cd "$2" && find -type f -exec md5sum {} \;)
}

diffdir2() {
    [[ "$1" ]] && [[ "$2" ]] || { echo "$0 <dir1> <dir2>" ; return 1 ; }
    comm -13 <(cd "$1" && find -type f | sort -g) <(cd "$2" && find -type f | sort -g)
}

bm() {
    case "$1" in
        dev)
            cd "$HOME/development/projects"
            ;;
        dot)
            cd "$HOME/dotfiles"
            ;;
        *)
            echo "unknown target"
            ;;
    esac
}

man() {
        env LESS_TERMCAP_mb=$'\E[01;31m' \
        LESS_TERMCAP_md=$'\E[01;38;5;74m' \
        LESS_TERMCAP_me=$'\E[0m' \
        LESS_TERMCAP_se=$'\E[0m' \
        LESS_TERMCAP_so=$'\E[38;5;246m' \
        LESS_TERMCAP_ue=$'\E[0m' \
        LESS_TERMCAP_us=$'\E[04;38;5;146m' \
        man "$@"
}

embiggen() {
    enscript --no-header --media=A4 --landscape --font="DejaVuSansMono30" -o - | ps2pdf - | zathura -
}

resolvecd() {
    cd "$(readlink -f $(pwd))"
}
