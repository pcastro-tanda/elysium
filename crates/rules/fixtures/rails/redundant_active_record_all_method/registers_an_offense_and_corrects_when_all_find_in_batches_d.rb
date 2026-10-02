User.all.find_in_batches(&:do_something)
     ^^^ Redundant `all` detected.
