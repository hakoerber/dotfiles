dotfiles
========

My configuration files.

Installation
------------

1. ``git clone git://github.com:whatevsz/dotfiles ~/dotfiles``
2. ``bash ~/dotfiles/scripts/setup.bash``

``setup.bash`` will back up all configuration files that would otherwise
be overridden and then symlink the content of all folders specified in $symlink_folders
into $HOME or the desired destination given in MAPPING, if present.

If you want to use a different directory instead of ``~/dotfiles``, just alter the first
line and replace  ``~/dotfiles`` with the desired destination and change the line
``config_dir="$HOME/dotfiles/"`` in ``scripts/setup.bash`` accordingly. You can
also choose a different folder for the backup of old files (default being ``~/.dotfiles.bak``)
by altering ``backup_dir`` in ``setup.bash`` to your needs.

Structure
---------

- ``scripts/`` - Scripts, e.g.  for setting up the configuration.
- ``setup/`` - Setup information, e.g. a list of packages.
- ``MAPPING`` - File that contains mapping directives.
- ``TODO`` - Some stuff I am to lazy to do right now ;).
- ``README.rst`` - The stuff you are reading right now.
- All other folders - These are the folders that contain the configuration files.

Mapping
-------

If you have configuration folder that is not located directly in $HOME,
but some subfolder (~/.config being a popular example), you
can tell the setup script to place the contents of that folder into an
arbitrary subfolder of $HOME. To do so, place an entry into ``MAPPING``. It has
the following format::

    <name of the configuration folder>::<root for that folder relative to $HOME>

Example::

    terminator::.config/

This will symlink the contents of the folder ``dotfiles/terminator`` into ``~/.config/``

When you provide multiple lines for the same folder, the first one that matches
will be used.

Required third party software
-----------------------------

- ``vim`` uses the plugin manager "vundle", available at https://github.com/gmarik/vundle
