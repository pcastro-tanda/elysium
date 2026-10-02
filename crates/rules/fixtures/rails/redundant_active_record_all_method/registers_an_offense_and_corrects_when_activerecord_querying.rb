do_something(User.all.order(:created_at))
                  ^^^ Redundant `all` detected.
