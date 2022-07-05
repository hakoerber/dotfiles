#!/usr/bin/env bash

set -o nounset
set -o errexit
set -o pipefail
set -o xtrace

cmds=()

workspacescratch="${1}"

i3msgworkspaces="$(i3-msg -t get_workspaces)"

output_of_pim=$(printf '%s' "${i3msgworkspaces}" | jq -r 'map(select(.name == "'"${workspacescratch}"'")) | .[0].output')

active_workspace_on_target_output=$(printf '%s' "${i3msgworkspaces}" | jq -r 'map(select(.output == "'"${output_of_pim}"'" and .visible)) | .[0].name')

focused_workspace_name=$(printf '%s' "${i3msgworkspaces}" | jq -r 'map(select(.focused)) | .[0].name')
focused_workspace_output=$(printf '%s' "${i3msgworkspaces}" | jq -r 'map(select(.focused)) | .[0].output')

if [[ "${focused_workspace_name}" == "${workspacescratch}" ]]; then
    exit 0
fi

cmds+=('unmark _destination')
cmds+=('mark _source')

if [[ "${active_workspace_on_target_output}" != "${workspacescratch}" ]] && [[ "${output_of_pim}" != "${focused_workspace_output}" ]]; then
    need_output_reset=1
else
    need_output_reset=0
fi

if ((need_output_reset)); then
    cmds+=('workspace "'"${active_workspace_on_target_output}"'"')
    cmds+=("mark --add _origin")
fi

cmds+=('workspace "'"${workspacescratch}"'"')
cmds+=('mark --add _destination')

cmds+=('[con_mark="^_destination$"] swap container with mark "_source"')

cmds+=('[con_mark="^_source$"] focus')
cmds+=('unmark _source')

if ((need_output_reset)); then
    cmds+=('[con_mark="^_origin$"] focus')
    cmds+=('unmark _origin')
fi

cmds+=('[con_mark="^_destination$"] focus')
cmds+=('unmark _destination')

i3msgcmd=""
for cmd in "${cmds[@]}"; do
    i3msgcmd="${i3msgcmd}${cmd};"
done
i3-msg "${i3msgcmd}"
