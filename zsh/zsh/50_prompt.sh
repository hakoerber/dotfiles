autoload -Uz vcs_info

_vcsbase="%{$fg[red]%}[%r] %{$fg[blue]%}[%{%B%}%b%{$fg[red]%}%m%{$fg[blue]%}] %{$fg[red]%}%{%B%}%c%u"

zstyle ':vcs_info:*' stagedstr 'I'
zstyle ':vcs_info:*' unstagedstr 'M'

zstyle ':vcs_info:*' enable git
zstyle ':vcs_info:*' check-for-changes true
zstyle ':vcs_info:*' get-revision true

zstyle ':vcs_info:git*' formats "$_vcsbase"
zstyle ':vcs_info:git*' actionformats "%{$fg[red]%}(%a) $_vcsbase"

zstyle ':vcs_info:git*+set-message:*' hooks git-untracked git-st git-remotebranch

+vi-git-untracked() {
    if [[ $(git rev-parse --is-inside-work-tree 2> /dev/null) == 'true' ]] && \
    [[ $(git ls-files --other --directory --exclude-standard | sed q | wc -l | tr -d ' ') == 1 ]] ; then
        hook_com[unstaged]+='?%f'
    fi
}

+vi-git-st() {
    local ahead behind
    local -a gitstatus

    ahead=$(git rev-list ${hook_com[branch]}@{upstream}..HEAD 2>/dev/null | wc -l)
    (( $ahead )) && gitstatus+=( "+${ahead}" )

    behind=$(git rev-list HEAD..${hook_com[branch]}@{upstream} 2>/dev/null | wc -l)
    (( $behind )) && gitstatus+=( "-${behind}" )

    hook_com[misc]+=${(j:/:)gitstatus}
}

+vi-git-remotebranch() {
    local remote

    remote=${$(git rev-parse --verify ${hook_com[branch]}@{upstream} \
        --symbolic-full-name 2>/dev/null)/refs\/remotes\/}

    if [[ -n ${remote} ]] ; then
        hook_com[branch]="${hook_com[branch]} %b%{$fg[magenta]%}<${remote}>"
    fi
}

precmd() {
    vcs_info
}

_topstr='%{$fg[green]%}%n@%m%{$fg[white]%}  ─  %{%B$fg[yellow]%}%~%{%b%} ${vcs_info_msg_0_}%{$fg[white]%} '
botstr='%B${PINFO}%#%b '

if _remote ; then
    _topstr="%{$fg[red]%}[remote]%{$fg[white]%} ${_topstr}"
fi

setopt prompt_subst
PROMPT='%{$fg[white]%}┌─ '"${_topstr}"'
└─ '"${botstr}"
RPROMPT="%{$fg[cyan]%}%*%{$fg[white]%} ─ [%?]"
