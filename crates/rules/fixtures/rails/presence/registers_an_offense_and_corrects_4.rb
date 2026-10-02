if value.present?
^^^^^^^^^^^^^^^^^ Use `value.presence || do_something(arg1, arg2)` instead of `if value.present? ... end`.
  value
else
  do_something arg1, arg2
end
