setprompt() {
    setopt prompt_subst

    topstr="%{$fg[green]%}%n@%m%{$fg[white]%}  ─  %{%B$fg[yellow]%}%~${vcs_info_msg_0_}%{%b%} %{$fg[white]%}"
    botstr="%B${PINFO}%#%b "

    if _remote ; then
        topstr="%{$fg[red]%}[remote]%{$fg[white]%} ${topstr}"
    fi

    PROMPT="%{$fg[white]%}┌─ ${topstr}
└─ ${botstr}"
    RPROMPT="%{$fg[cyan]%}%*%{$fg[white]%} ─ [%?]"
}

setprompt
