#!/usr/bin/env bash

printf '%s' "starting gpg-agent"
systemd-run -r --user --setenv=DISPLAY=${DISPLAY} gpg-agent --homedir "$HOME/.gnupg" --no-detach --daemon
