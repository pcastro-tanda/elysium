User.all.in_batches(&:do_something)
     ^^^ Redundant `all` detected.
