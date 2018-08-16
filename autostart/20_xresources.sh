#!/usr/bin/env bash

printf '%s' "parsing .Xresources"
xrdb -merge -I${HOME} ~/.Xresources
