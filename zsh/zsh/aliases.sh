### TRANSLATIONS
alias vim="nvim -u $VIMRC"

### COMMON OPERATIONS
alias ll='ls -AlFh'
alias la='ls -A'

alias spm="sudo pacman"

alias tml="tmux list-sessions"
alias tma="tmux ls 2>/dev/null && tmux attach-session || tmux"

alias clip="xclip -selection clipboard"
alias clipo="xclip -out -selection clipboard"

alias rgrep="grep -r"

alias vimrc="vim -c ':e \$MYVIMRC'"
alias zshrc="vim -c ':e ~/.zshrc' ; source ~/.zshrc"

alias calc='python3 -ic "from math import *; import cmath"'

alias le_haxxor_1='clear && dmesg | pv -qL 20'
alias le_haxxor_2='clear && hexdump -C /dev/urandom | pv -qlL 2'

alias b='cd $OLDPWD'

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

# show non-printable characters by default
alias cat="cat -v"

### SHORTENING COMMAND NAMES
alias cs="cryptsetup"
alias v="vim"
alias g="git"
alias t="tmux"
alias c="cd"
alias l="ls"
alias s="sudo"
alias t="tmux"
alias cl="clear"
