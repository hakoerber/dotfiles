# dotfiles

My configuration files.

# Installation

Because it manages multiple users on the system, the directory is supposed to be
at `/var/lib/dotfiles`.

To setup the dotfiles:

1. `git clone https://github.com/hakoerber/dotfiles.git ~/dotfiles`
2. `cd ~/dotfiles && ./install.sh`

# Partial application

To apply only a subset of the changes, use ansible tags that are available via
the Makefile:

| Command | Description |
| --- | --- |
| `make update` | Updates the system with the latest packages |
| `make packages` | Installs all defined packages (see `packages.yml`) |
| `make dotfiles` | Manages the users' dotfiles |

Note that these are not supported on a first bootstrap run. Only use them after
the bootstrap to update existing configuration.
