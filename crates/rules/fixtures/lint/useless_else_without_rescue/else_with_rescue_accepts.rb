begin
  do_something
rescue ArgumentError
  handle_argument_error
else
  handle_unknown_errors
end
