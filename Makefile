ansible_run = ansible-playbook --inventory localhost, --diff ./playbook.yml ${ANSIBLE_EXTRA_ARGS}

.PHONY: all
all:
	$(ansible_run)

.PHONY: config
config:
	$(ansible_run) --skip-tags system-update

.PHONY: system-update
system-update:
	$(ansible_run) --tags system-update

.PHONY: reboot
reboot:
	sudo reboot

.PHONY: poweroff
poweroff:
	sudo poweroff

.PHONY: weekend
weekend: | update poweroff

.PHONY: packages
packages:
	$(ansible_run) --tags packages

.PHONY: dotfiles
dotfiles:
	$(ansible_run) --tags dotfiles

.PHONY: test
test:
	./test-in-docker.sh

.PHONY: fmt
fmt:
	git ls-files -z '*.md'   | xargs -0 prettier --print-width 80 --prose-wrap always --write
	git ls-files -z '*.toml' | xargs -0 taplo format
