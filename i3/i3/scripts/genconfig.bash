#!/bin/bash

host="$(hostname --short)"

# main configuration file that is always used
MAIN_CONF="$HOME/.i3/config"
# user-specific /var
VARDIR="$HOME/.var"
# user-specific /var/run
RUNDIR="$VARDIR/run"
# temporary configuration file used for this session
SESSION_CONF="$RUNDIR/i3/${host}.config"
# directory that contains host specific configuration
CONF_DIR="$HOME/.i3/config.d"
# file that should be used when no host specific configuration present
DEFAULT_CONF="$CONF_DIR/default.config"

LOGFILE="$LOGDIR/i3/genconfig.log"

host_specific_conf="$CONF_DIR/$host.config"

log() {
    echo "[$(date +%FT%T)] $*" >> "$LOGFILE"
}

# if it's a symlink to $MAIN_CONF, cat will fail
[[ -f "$SESSION_CONF" ]] && rm "$SESSION_CONF"

if [[ ! -f "$host_specific_conf" ]] && [[ ! -f "$DEFAULT_CONF" ]]; then
    # if there is no host-specific configuration and no default one, just use
    # the main config
    log "neither config for host $host at $host_specific_conf nor default config at $DEFAULT_CONF found, using main only"
    ln -sf "$MAIN_CONF" "$SESSION_CONF"
else
    # either use the host specific config if present, or the default if not
    if [[ -f "$host_specific_conf" ]]; then
        log "found config for host $host at $host_specific_conf"
        conf_to_use="$host_specific_conf"
    else
        log "no config for host $host found, using default one"
        conf_to_use="$DEFAULT_CONF"
    fi
    cat "$MAIN_CONF" <(echo -e "\n###\n### host-specific configuration for host \"$host\"\n###\n") "$conf_to_use" > "$SESSION_CONF"
    log "created session config at $SESSION_CONF"
fi

echo "$SESSION_CONF"

# if we got any parameters, tell i3 to reload the config
# so the script can be used both on startup without reload (as i3 is not even
# running yet) and later when reloading
if [[ -n "$1" ]] ; then
    log "telling i3 to reload the config file"
    i3-msg reload
fi
