begin
rescue => error
          ^^^^^ Use `e` instead of `error`.
  do_something(error: error)
end
