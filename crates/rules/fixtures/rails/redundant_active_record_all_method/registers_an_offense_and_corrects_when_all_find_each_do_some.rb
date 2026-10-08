User.all.find_each(&:do_something)
     ^^^ Redundant `all` detected.
