#!/usr/bin/env python3

import yaml
from pprint import pprint

package_file = "./packages.yml"

apps = yaml.safe_load(open(package_file, 'r'))

missing_config = {}

for appname, appconfig in apps['packages']['list'].items():
    for distro, packagelist in appconfig.items():
        if len(packagelist) == 0:
            if distro not in missing_config.keys():
                missing_config[distro] = []
            missing_config[distro].append(appname)

print(yaml.dump(missing_config), end="")
