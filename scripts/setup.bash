#!/usr/bin/env bash
# Possible options:
# --dry-run -n : only show what would be done without doing anything

# config directory
config_dir="$HOME/dotfiles/"

mapping_file="$config_dir/MAPPING"

skel_dir="$config_dir/skel/"

# backup directory, files that would otherwise be overwritten go there
backup_dir="$HOME/.dotfiles.bak/"

# these folders inside $config_dir will be ignored
ignore_folders=('scripts' 'skel')

MAPPING_SEPARATOR='::'

dryrun=0

while [[ $# -gt 0 ]] ; do
    case "$1" in
        -n|--dry-run)
            echo "dry run"
            dryrun=1
            ;;
        *)
            echo "invalid parameters"
            exit 1
            ;;
    esac
    shift
done

path_combine()
{
    echo "$(dirname "$1")/$(basename "$1")/$2"
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
while IFS= read -d $'\0' -r folder ; do
    folder="$(basename "$folder")"
    ignored=0
    for ignore in "${ignore_folders[@]}" ; do
        if [[ "$ignore" == "$folder" ]] ; then
            ignored=1
        fi
    done
    if (( "$ignored" )) ; then
        continue
    fi
    mapping="$(get_mapping "$folder")"


    source_folder="$(path_combine "$config_dir" "$folder")"
    [[ "$(ls "$source_folder")" ]] || continue
    for file in "$source_folder"/* ; do
        if [[ -z "$mapping" ]]; then
            destination="$(path_combine "$DEFAULT_ROOT" ".$(basename "$file")")"
        else
            destination="$(path_combine "$(path_combine "$HOME" "$mapping")" "$(basename "$file")")"
        fi
	if [[ "$(readlink "$destination")" == "$file" ]]; then
            continue
        elif [[ -e "$destination" ]]; then
            backup_destination="$(path_combine "$backup_dir" "$(basename "$destination")")"
            if ! [[ -d "$backup_dir" ]] ; then
                echo "mkdir -p \"$backup_dir\""
                (( $dryrun )) || mkdir -p "$backup_dir"
            fi
            echo "mv \"$destination\" -> \"$backup_destination\""
            (( $dryrun )) || mv "$destination" "$backup_destination"
        fi

        if [[ ! -e "$(dirname "$destination")" ]] ; then
            mkdir "$(dirname "$destination")"
        fi
        [[ -e "$destination" ]] && [[ "$(readlink "$destination")" == "$file" ]] && continue
        echo "ln -sf \"$file\" -> \"$destination\""
        (( $dryrun )) || ln -sf "$file" "$destination"
    done
done < <(find "$config_dir"/* -maxdepth 0 -mindepth 0 -type d -print0)

(( $dryrun )) || rsync --archive --verbose --ignore-existing "$skel_dir/" "$HOME"
