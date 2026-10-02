if value.present?
^^^^^^^^^^^^^^^^^ Use `value.presence || do_something(value)` instead of `if value.present? ... end`.
  value
else
  do_something value
end
