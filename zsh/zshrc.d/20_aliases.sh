### TRANSLATIONS
alias vim="nvim"

alias urxvt="urxvt256c"

### COMMON OPERATIONS
alias ll='ls -AlFh'
alias la='ls -A'

alias spm="sudo pacman"

alias tml="tmux list-sessions"
alias tma="tmux ls 2>/dev/null && tmux attach-session || tmux"
alias tmn="tmux new-session -A -s"

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
