FOO::Const&.do_something
          ^^ Redundant safe navigation detected, use `.` instead.
bar::ConstName&.do_something
              ^^ Redundant safe navigation detected, use `.` instead.
BAZ::Const_name&.do_something # It is treated as camel case, similar to the `Naming/ConstantName` cop.
               ^^ Redundant safe navigation detected, use `.` instead.
