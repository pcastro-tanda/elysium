foo.to_h { |entry| do_something(entry) }&.keys
                                        ^^ Redundant safe navigation detected, use `.` instead.
