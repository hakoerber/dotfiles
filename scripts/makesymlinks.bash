#!/usr/bin/env bash

# config directory
config_dir="$HOME/config/"

# backup directory, files that would otherwise be overwritten go there
backup_dir="$HOME/oldconfig/"

# the following folders inside $config_dir will be inspected and symlinked:
symlink_folders="bash git i3 vim zsh"

# backup the old config files
backup_dir="$backup_dir/$(date +%Y-%m-%dT%H:%M:%S)"
echo "Backing up old configuration files into \"$backup_dir\"."
mkdir -p "$backup_dir"
for folder in $symlink_folders ; do
    for file in "$config_dir/$folder"/* ; do
        oldfile="$HOME/.$(basename "$file")"
        if [[ -e "$oldfile" ]] ; then
            destination="$backup_dir/$(basename "$oldfile")"
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
    for file in "$config_dir/$folder"/* ; do
        destination="$HOME/.$(basename "$file")"
        echo "ln -s \"$file\" -> \"$destination\""
        ln -s "$file" "$destination"
    done
done

