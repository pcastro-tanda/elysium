begin
  do_something
rescue StandardError => error
                        ^^^^^ Use `e` instead of `error`.
  error = build_message(error)
end
puts error
