venv = ./venv
requirements = requirements.txt
activate = . $(venv)/bin/activate
pip = pip
ansible = venv/bin/ansible-playbook

.PHONY: all
install: $(ansible)
	$(activate) && ansible-playbook --inventory localhost, --diff --verbose ./playbook.yml

.PHONY: clean
clean:
	rm -rf venv

$(ansible): venv

venv:
	python3 -m venv $(venv)
	$(activate) && $(pip) install -r $(requirements)

freeze:
	$(activate) && $(pip) freeze > $(requirements)
