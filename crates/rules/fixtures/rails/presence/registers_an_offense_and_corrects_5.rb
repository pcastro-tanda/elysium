if value.present?
^^^^^^^^^^^^^^^^^ Use `value.presence || foo.do_something(value)` instead of `if value.present? ... end`.
  value
else
  foo.do_something value
end
