until foo&.respond_to?(:bar)
         ^^ Redundant safe navigation detected, use `.` instead.
  do_something
end

begin
  do_something
end until foo&.respond_to?(:bar)
             ^^ Redundant safe navigation detected, use `.` instead.
