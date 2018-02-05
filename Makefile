venv = ./venv
requirements = requirements.txt
activate = source $(venv)/bin/activate
pip = pip
ansible = venv/bin/ansible-playbook

.PHONY: all
install: $(ansible)
	$(activate) && ansible-playbook --diff --verbose ./playbook.yml

.PHONY: clean
clean:
	rm -r venv

$(ansible): venv

venv:
	command -v virtualenv || sudo dnf install -y python2-virtualenv
	virtualenv --system-site-packages --python=python2 $(venv)
	$(activate) && $(pip) install -r $(requirements)

freeze:
	$(activate) && $(pip) freeze > $(requirements)
