do_something if foo&.respond_to?(:bar) && !foo&.respond_to?(:baz)
                                              ^^ Redundant safe navigation detected, use `.` instead.
                   ^^ Redundant safe navigation detected, use `.` instead.
