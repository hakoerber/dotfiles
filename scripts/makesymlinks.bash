#!/usr/bin/env bash

# config directory
config_dir="$HOME/dotfiles/"

# backup directory, files that would otherwise be overwritten go there
backup_dir="$HOME/dotfiles.bak/"

# the following folders inside $config_dir will be inspected and symlinked:
symlink_folders="bash git i3 vim zsh conky"

path_combine()
{
    echo "$(dirname "$1")/$(basename "$1")/$(basename "$2")"
}

# backup the old config files
backup_dir="$backup_dir/$(date +%Y-%m-%dT%H:%M:%S)"
echo "Backing up old configuration files into \"$backup_dir\"."
mkdir -p "$backup_dir"
for folder in $symlink_folders ; do
    for file in "$(path_combine "$config_dir" "$folder")"/* ; do
        oldfile="$(path_combine "$HOME" ".$(basename "$file")")"
        if [[ -e "$oldfile" ]] ; then
            destination="$(path_combine "$backup_dir" "$(basename "$oldfile")")"
            echo "mv: \"$oldfile\" -> \"$destination\""
            mv "$oldfile" "$destination"
        else
            echo "\"$oldfile\" not found, skipped."
        fi
    done
done

# now symlink the files in $config_dir into the home directory
echo "Creating symlinks for configuration files in \"$config_dir\"."
for folder in $symlink_folders ; do
    for file in "$(path_combine "$config_dir" "$folder")"/* ; do
        destination="$(path_combine "$HOME" ".$(basename "$file")")"
        echo "ln -sf \"$file\" -> \"$destination\""
        ln -sf "$file" "$destination"
    done
done

