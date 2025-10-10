# Make sure to standardize locale, regardless of the machine config
#
# Having a different locale broke "yes | pacman -S" to force-install
# iptables, for example
export LC_ALL = en_US.UTF-8

ansible_run = ansible-playbook --inventory localhost, --diff ./playbook.yml ${ANSIBLE_EXTRA_ARGS}

.PHONY: config
config:
	$(ansible_run)

.PHONY: maintenance
maintenance:
	./maintenance.sh

.PHONY: test
test:
	./test-in-docker.sh

.PHONY: fmt
fmt:
	git ls-files -z '*.md'   | xargs -0 prettier --print-width 80 --prose-wrap always --write
	git ls-files -z '*.toml' | xargs -0 taplo format
