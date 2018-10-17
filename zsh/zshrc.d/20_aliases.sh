alias vim="nvim"

### COMMON OPERATIONS
alias ll='ls -AlFh'
alias la='ls -A'

alias spm="sudo pacman"

alias tml="tmux list-sessions"
alias tma="tmux ls 2>/dev/null && tmux attach-session || tmux"
alias tmn="tmux new-session -A -s"

alias clipc="xclip -selection primary"
alias clipo="xclip -out -selection clipboard"

alias rgrep="grep -r"

alias vimrc="vim -c ':e \$MYVIMRC'"
alias zshrc="vim -c ':e ~/.zshrc' ; source ~/.zshrc"

alias calc='python3 -ic "from math import *; import cmath"'

alias le_haxxor_1='clear && dmesg | pv -qL 20'
alias le_haxxor_2='clear && hexdump -C /dev/urandom | pv -qlL 2'

alias root='sudo -sE'

### USEFUL DEFAULT OPTIONS
alias tmux="tmux -2"

alias chmod="chmod -c"
alias chown="chown -c"

alias ls="ls --group-directories-first --classify --color=auto"

alias grep='grep --color=auto'
alias fgrep='fgrep --color=auto'
alias egrep='egrep --color=auto'

alias rm='rm -v'
alias cp='cp -vi'
alias mv='mv -vi'
alias ln='ln -v'

alias du='du -h'
alias df='df -h'

### SHORTENING COMMAND NAMES
alias cs="cryptsetup"
alias v="vim"
alias g="git"
alias c="cd"
alias l="ls"
alias s="sudo"
alias cl="clear"

alias nocolor="sed -r \"s/\x1B\[([0-9]{1,2}(;[0-9]{1,2})?)?[mGK]//g\""

alias ip="ip -color"

alias vimtask="vim -c :TW"

alias tw="task"
alias twl="task list"
alias twa="task add"
alias twd="task done"
alias inbox="task add +inbox"

alias yaml2js="python -c 'import sys, yaml, json; json.dump(yaml.load(sys.stdin), sys.stdout, indent=4)'"

alias currentbranch='git rev-parse --abbrev-ref HEAD'
alias gpush='git push origin $(currentbranch)'

alias pass=mypass

alias issh="ssh -o StrictHostKeyChecking=false -o UserKnownHostsFile=/dev/null"
alias gfix='git commit --amend --no-edit'
alias gfixa='git commit --amend --no-edit --all '
alias gfixp='git commit --amend --no-edit --patch'

alias issh="ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null"

alias newpw="pwgen --secure 25 1"

gitmaster() {
    git stash push -m gitmaster-$(date -uIseconds) -u || return 1
    _branch=$(git rev-parse --abbrev-ref HEAD)
    git fe || return 1
    git checkout master || return 1
    git merge origin/master || return 1
    git checkout $_branch
    git stash pop
}

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

clip() {
    tee >(xclip -selection clipboard) | tee >(xclip -selection primary)
}

gb() {
    _superproject="$(git rev-parse --show-superproject-working-tree)"
    if [[ -n "${_superproject}" ]] ; then
        builtin cd "${_superproject}"
    else
        builtin cd "$(git rev-parse --show-toplevel)"
    fi
}
