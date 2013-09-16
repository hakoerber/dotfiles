dotfiles
========

My configuration files.

Installation
------------

1. ``git clone git://github.com:whatevsz/dotfiles ~/dotfiles``
2. ``bash ~/dotfiles/scripts/makesymlinks.bash``

``makesymlinks,bash`` will back up all configuration files that would otherwise
be overridden and then symlink the content of all folders specified in $symlink_folders
into $HOME or the desired destination given in MAPPING, if present.

If you want to use a different directory instead of ``~/dotfiles``, just alter the first
line and replace  ``~/dotfiles`` with the desired destination, and change the line
``config_dir="$HOME/dotfiles/"`` in ``scripts/makesymlinks.bash`` accordingly. You can
also choose a different folder for the backup of old files (default being ``~/.dotfiles,bak``
by altering ``backup_dir`` in ``scripts/makesymlinks.bash`` to your needs.

Structure
---------

- ``scripts/`` - Scripts for setting up the configuration.
- ``MAPPING`` - File that contains mapping directives.
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

This will place the contents of the folder ``dotfiles/terminator`` into ``~/.config/``

When you provide multiple lines for the same folder, the first one that will match
will be used.
