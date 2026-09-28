obj.attr = do_something do
  rescue StandardError
  ^^^^^^ `rescue` at 2, 2 is not aligned with `obj` at 1, 0.
end
