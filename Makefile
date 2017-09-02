venv = ./venv
requirements = requirements.txt
activate = source $(venv)/bin/activate
pip = pip

.PHONY: all
all: dotfiles packages

venv:
	virtualenv --system-site-packages --python=python2 $(venv)
	$(activate) && $(pip) install -r $(requirements)

freeze:
	$(activate) && $(pip) freeze > $(requirements)

.PHONY: dotfiles
dotfiles:
	./install

.PHONY: packages
packages: venv
	$(activate) && ansible-playbook install-packages.yml
