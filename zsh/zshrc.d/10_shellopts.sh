setopt EXTENDED_HISTORY
setopt HIST_EXPIRE_DUPS_FIRST
setopt HIST_VERIFY
setopt INC_APPEND_HISTORY
setopt SHARE_HISTORY
setopt AUTO_CD
setopt APPEND_HISTORY
setopt HIST_IGNORE_DUPS
setopt NOHIST_IGNORE_ALL_DUPS
setopt HIST_IGNORE_SPACE
setopt CORRECT
setopt RM_STAR_SILENT
setopt BG_NICE
setopt CHECK_JOBS
setopt HUP
setopt LONG_LIST_JOBS

bindkey -e

autoload -U promptinit
promptinit

autoload -U colors
colors

autoload -U compinit
compinit
zstyle ':completion:*' menu select
setopt completealiases

autoload -U edit-command-line
zle -N edit-command-line
bindkey '^xe' edit-command-line
bindkey '^x^e' edit-command-line

HISTSIZE=10000
SAVEHIST=10000
HISTFILE="$HOME/.zsh_history"

autoload -Uz vcs_info
zstyle ':vcs_info:*' enable git hg
zstyle ':vcs_info:*' check-for-changes true
zstyle ':vcs_info:git*' formats "%{${fg[cyan]}%}[%{${fg[green]}%}%s%{${fg[cyan]}%}][%{${fg[blue]}%}%r/%S%%{${fg[cyan]}%}][%{${fg[blue]}%}%b%{${fg[yellow]}%}%m%u%c%{${fg[cyan]}%}]%{$reset_color%}"
