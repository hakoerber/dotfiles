# dotfiles

My configuration files for my systems. Uses Ansible for local configuration.

## Supported OS

Only Arch Linux is supported

## Bootstrapping

Bootstrapping is specific to the exact machine that is installed. See
`_machines/` for machine-specific configuration, and `install_scripts/` for the
machine install scripts.

They are keyed by hostname.

For easier installation, the install scripts are available via shortlinks. To
(re)install a new machine from a Arch live environment:

```
curl --proto '=https' -O -sSfL https://s.hkoerber.de/i/bootstrap.sh && bash bootstrap.sh {host}
```

## Manual Installation

Because it manages multiple users on the system, the directory is supposed to be
at `/var/lib/dotfiles`.

To set up the dotfiles:

1. `git clone https://github.com/hakoerber/dotfiles.git ~/dotfiles`
2. `cd ~/dotfiles && ./install.sh`

## Partial application

To apply only a subset of the changes, use ansible tags that are available via
the Makefile:

| Command         | Description                                        |
| --------------- | -------------------------------------------------- |
| `make packages` | Installs all defined packages (see `packages.yml`) |
| `make dotfiles` | Manages the users' dotfiles                        |

Note that these are not supported on a first bootstrap run. Only use them after
the bootstrap to update existing configuration.
