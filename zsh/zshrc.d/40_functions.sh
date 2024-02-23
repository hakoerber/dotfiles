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
    enscript --no-header --media=A4 --landscape --font="DejaVuSansMono30" -o - 2>/dev/null | ps2pdf - | zathura -
}

resolvecd() {
    cd "$(readlink -f $(pwd))"
}

ssht () {
    ssh -t $@ "tmux a || tmux";
}

t() {
    if [[ "$1" ]] ; then
        tmux new-session -A -s "$1"
    else
        tmux attach-session
    fi
}

b() {
    bookmarks=${DOTFILES}/bookmarks
    bookmark="$1"
    if ! [[ "${bookmark}" ]] ; then
        printf 'Need a bookmark' >&2
        return 1
    fi
    if ! [[ -r "${bookmark}" ]] ; then
        printf 'Invalid bookmark %s' "${bookmark}" >&2
        return 1
    fi
    target="$(head -1 ${bookmark})"
    if ! [[ -e "${target}" ]] ; then
        printf 'Traget not found: %s' "${target}" >&2
    fi
    cd "$(eval ${target})"
}

sb() {
    echo $(( $1 * $(cat /sys/class/backlight/intel_backlight/max_brightness) / 100)) | sudo tee /sys/class/backlight/intel_backlight/brightness
}

journal() {
    journaldir=~/journal/
    file=~/journal/$(date +%Y-%m-%d).md
    if [[ ! -e $file ]] ; then
        cp $journaldir/template.md $file || return
    fi
    $EDITOR $file
}

syncfolder() {
    folder=$1
    mv $folder ~/sync
    ln -s ~/sync/$folder ~/$folder
}

prefix() {
    prefix=$2
    file=$1
    mv $file ${prefix}${file}
}

tmp() {
    cd "$(mktemp -d)"
}

kubectl_pod() {
    kubectl-env mycloud get -n "${1}" pods --field-selector=status.phase=Running --selector=${2} -o jsonpath='{.items[*].metadata.name}'
}

kubectl_deployment() {
    kubectl-env mycloud get -n "${1}" deployment --selector=${2} -o jsonpath='{.items[*].metadata.name}'
}

# The semver_ checks are inspired by
# https://stackoverflow.com/questions/4023830/how-to-compare-two-strings-in-dot-separated-version-format-in-bash
semver_lte() {
    v1="${1}"
    v2="${2}"

    printf '%s\n%s' "${v1}" "${v2}" | sort --version-sort --check=silent
}

semver_lt() {
    v1="${1}"
    v2="${2}"

    semver_lte "${v1}" "${v2}" && [[ ! "${v1}" == "${v2}" ]]
}

semver_gte() {
    v1="${1}"
    v2="${2}"

    ! semver_lt "${v1}" "${v2}"
}

semver_gt() {
    v1="${1}"
    v2="${2}"

    ! semver_lte "${v1}" "${v2}"
}
