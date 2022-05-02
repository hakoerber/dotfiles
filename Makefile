venv = ./venv
requirements = requirements.txt
activate = . $(venv)/bin/activate
pip = pip
ansible = venv/bin/ansible-playbook
ansible_run = $(activate) && ansible-playbook -e ansible_python_interpreter=/usr/bin/python3 --inventory localhost, --diff --verbose ./playbook.yml ${ANSIBLE_EXTRA_ARGS}

.PHONY: config
config: | venv $(ansible)
	$(ansible_run) --skip-tags system-update

.PHONY: system-update
system-update: $(ansible)
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
