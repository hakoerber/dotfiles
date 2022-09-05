#!/usr/bin/env bash

set -o nounset
set -o pipefail
set -o errexit

codium --install-extension ms-vscode.cpptools
codium --install-extension ms-python.python
codium --install-extension rust-lang.rust-analyzer
codium --install-extension asvetliakov.vscode-neovim
codium --install-extension monokai.theme-monokai-pro-vscode
codium --install-extension yzhang.markdown-all-in-one
codium --install-extension puppet.puppet-vscode


