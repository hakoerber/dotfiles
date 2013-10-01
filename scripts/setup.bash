#!/usr/bin/env bash

# config directory
config_dir="$HOME/dotfiles/"

mapping_file="$config_dir/MAPPING"

# backup directory, files that would otherwise be overwritten go there
backup_dir="$HOME/.dotfiles.bak/"

# the following folders inside $config_dir will be inspected and the
# contents symlinked:
symlink_folders='bash git i3 vim zsh conky terminator x mpd ncmpcpp'

MAPPING_SEPARATOR='::'

path_combine()
{
    echo "$(dirname "$1")/$(basename "$1")/$(basename "$2")"
}

DEFAULT_ROOT="$HOME"
# returns "" if no mapping necessary
# format of the MAPPING file:
# folder::root
get_mapping()
{
    entry="$(grep -E "^$1$MAPPING_SEPARATOR.+$" "$mapping_file" | head -n1 | grep -Eo "$MAPPING_SEPARATOR.+$" | cut -c 3-)"
    echo "$entry"
}


backup_dir="$backup_dir/$(date +%Y-%m-%dT%H:%M:%S)"

# backup the old config files, symlinks will be skipped
# then symlink the files in $config_dir into the home directory
for folder in $symlink_folders ; do
    mapping="$(get_mapping "$folder")"

    source_folder="$(path_combine "$config_dir" "$folder")"
    [[ "$(ls "$source_folder")" ]] || continue
    for file in "$source_folder"/* ; do
        if [[ -z "$mapping" ]]; then
            destination="$(path_combine "$DEFAULT_ROOT" ".$(basename "$file")")"
        else
            destination="$(path_combine "$(path_combine "$HOME" "$mapping")" "$(basename "$file")")"
        fi
        if [[ -L "$destination" ]]; then
            continue
        elif [[ -e "$destination" ]]; then
            backup_destination="$(path_combine "$backup_dir" "$(basename "$destination")")"
            [[ -d "$backup_dir" ]] || mkdir -p "$backup_dir"
            echo "mv \"$destination\" -> \"$backup_destination\""
            mv "$destination" "$backup_destination"
        fi

        [[ -e "$destination" ]] && [[ "$(readlink "$destination")" == "$file" ]] && continue
        echo "ln -sf \"$file\" -> \"$destination\""
        ln -sf "$file" "$destination"
    done
done

