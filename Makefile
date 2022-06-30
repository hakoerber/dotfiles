DISTRO := $(shell . /etc/os-release && echo $$NAME)

venv = ./venv
requirements = requirements.txt
activate = . $(venv)/bin/activate
pip = pip

ifeq ($(DISTRO),Ubuntu)
	ansible_run = $(activate) && ansible-playbook -e ansible_python_interpreter=/usr/bin/python3 --inventory localhost, --diff ./playbook.yml ${ANSIBLE_EXTRA_ARGS}
else
	ansible_run =ansible-playbook -e ansible_python_interpreter=/usr/bin/python3 --inventory localhost, --diff ./playbook.yml ${ANSIBLE_EXTRA_ARGS}
endif

.PHONY: all
all: | venv
	$(ansible_run)

.PHONY: config
config: | venv
	$(ansible_run) --skip-tags system-update

.PHONY: system-update
system-update: venv
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
packages: venv
	$(ansible_run) --tags packages

.PHONY: dotfiles
dotfiles: venv
	$(ansible_run) --tags dotfiles

.PHONY: clean
clean:
	rm -rf venv

.PHONY: test
test:
	./test-in-docker.sh

ifeq ($(DISTRO), Ubuntux)
venv:
		python3 -m venv $(venv)
		$(activate) && $(pip) install -r $(requirements)
else
venv:
	true
endif

.PHONY: freeze
freeze:
	$(activate) && $(pip) freeze > $(requirements)
