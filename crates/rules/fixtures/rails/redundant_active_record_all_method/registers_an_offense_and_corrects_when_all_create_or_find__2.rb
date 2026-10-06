User.all.create_or_find_by!(name: name)
     ^^^ Redundant `all` detected.
