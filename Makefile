# Make sure to standardize locale, regardless of the machine config
#
# Having a different locale broke "yes | pacman -S" to force-install
# iptables, for example
export LC_ALL = en_US.UTF-8

ansible_run = ansible-playbook --inventory localhost, --diff ./playbook.yml ${ANSIBLE_EXTRA_ARGS}

.PHONY: config
config:
	$(ansible_run)

.PHONY: fmt
fmt:
	git ls-files -z '*.md'   | xargs -0 prettier --print-width 80 --prose-wrap always --write
	git ls-files -z '*.toml' | xargs -0 taplo format
	git ls-files -z '*.py'   | xargs -0 ruff format
	git ls-files -z '*.sh'   | xargs -0 shfmt --write --indent 4 --space-redirects --case-indent

.PHONY: lint
lint:
	ansible-lint --force-color playbook.yml
	git ls-files -z '*.py' | xargs -0 ruff check
	git ls-files -z '*.sh' | xargs -0 shellcheck
