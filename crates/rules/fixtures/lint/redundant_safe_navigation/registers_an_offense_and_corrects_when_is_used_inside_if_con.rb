if foo&.respond_to?(:bar)
      ^^ Redundant safe navigation detected, use `.` instead.
  do_something
elsif foo&.respond_to?(:baz)
         ^^ Redundant safe navigation detected, use `.` instead.
  do_something_else
end
