#!/bin/bash

# main configuration file that is always used
MAIN_CONF="$HOME/.i3/config"
# temporary configuration file used for this session
SESSION_CONF="$HOME/.i3/session.config"
# directory that contains host specific configuration
CONF_DIR="$HOME/.i3/config.d"
# file that should be used when no host specific configuration present
DEFAULT_CONF="$CONF_DIR/default.config"

host_specific_conf="$CONF_DIR/$(hostname).config"

# if it's a symlink to $MAIN_CONF, cat will fail
[[ -f "$SESSION_CONF" ]] && rm "$SESSION_CONF"

if [[ ! -f "$host_specific_conf" ]] && [[ ! -f "$DEFAULT_CONF" ]]; then
    # if there is no host-specific configuration and no default one, just use
    # the main config
    ln -sf "$MAIN_CONF" "$SESSION_CONF"
else
    # either use the host specific config if present, or the default if not
    if [[ -f "$host_specific_conf" ]]; then
        conf_to_use="$host_specific_conf"
    else
        conf_to_use="$DEFAULT_CONF"
    fi
    cat "$MAIN_CONF" "$conf_to_use" > "$SESSION_CONF"
fi

echo "$SESSION_CONF"
