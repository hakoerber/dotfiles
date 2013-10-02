#!/usr/bin/env bash
# From http://i3wm.org/docs/user-contributed/conky-i3bar.html

path_conkyrc="$1"

# end the header so that i3bar knows we want to use JSON:
echo '{ "version" : 1 , "click_events" : true }'

# Begin the endless array.
echo '['

# We send an empty first array of blocks to make the loop simpler:
echo '[],'

# Now send blocks with information forever:
conky -c "$path_conkyrc"

while read input ; do
    bash ~/.i3/i3bar_clickparser.bash "$input"
done

