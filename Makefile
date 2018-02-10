venv = ./venv
requirements = requirements.txt
activate = . $(venv)/bin/activate
pip = pip
ansible = venv/bin/ansible-playbook
dotbot = _dotbot/bin/dotbot

.PHONY: all
install: $(ansible) submodules
	$(activate) && ansible-playbook --inventory localhost, --diff --verbose ./playbook.yml

.PHONY: submodules
submodules: $(dotbot)

$(dotbot):
	git submodule update --init _dotbot

.PHONY: clean
clean:
	rm -rf venv

$(ansible): venv

venv:
	command -v virtualenv || sudo dnf install -y python2-virtualenv || sudo apt-get -y install virtualenv
	virtualenv --no-site-packages --python=python2 $(venv)
	$(activate) && $(pip) install -r $(requirements)

freeze:
	$(activate) && $(pip) freeze > $(requirements)
