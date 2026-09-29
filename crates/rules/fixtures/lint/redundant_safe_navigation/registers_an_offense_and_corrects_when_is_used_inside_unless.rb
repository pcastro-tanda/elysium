do_something unless foo&.respond_to?(:bar)
                       ^^ Redundant safe navigation detected, use `.` instead.
