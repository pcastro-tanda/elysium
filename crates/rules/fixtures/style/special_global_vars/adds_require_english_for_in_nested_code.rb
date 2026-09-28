# frozen_string_literal: true

x = true
if x
  puts $$
       ^^ Prefer `$PROCESS_ID` or `$PID` from the stdlib 'English' module (don't forget to require it) over `$$`.
end
