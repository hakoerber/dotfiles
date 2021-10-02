venv = ./venv
requirements = requirements.txt
activate = . $(venv)/bin/activate
pip = pip
ansible = venv/bin/ansible-playbook
ansible_run = $(activate) && ansible-playbook --inventory localhost, --diff --verbose ./playbook.yml ${ANSIBLE_EXTRA_ARGS}

.PHONY: all
all: | venv $(ansible)
	$(ansible_run)

.PHONY: dryrun
dryrun: $(ansible)
	$(ansible_run) --check

.PHONY: update
update: $(ansible)
	$(ansible_run) --tags update_system

.PHONY: reboot
reboot:
	sudo reboot

.PHONY: poweroff
poweroff:
	sudo poweroff

.PHONY: weekend
weekend: | update poweroff

.PHONY: packages
packages: $(ansible)
	$(ansible_run) --tags packages

.PHONY: dotfiles
dotfiles: $(ansible)
	$(ansible_run) --tags dotfiles

.PHONY: clean
clean:
	rm -rf venv

.PHONY: test
test:
	./test-in-docker.sh

$(ansible): venv

venv:
	python3 -m venv $(venv)
	$(activate) && $(pip) install -r $(requirements)

.PHONY: freeze
freeze:
	$(activate) && $(pip) freeze > $(requirements)
