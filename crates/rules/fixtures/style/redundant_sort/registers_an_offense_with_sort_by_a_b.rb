x.sort_by { |y| y.foo || bar }.last
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `max_by` instead of `sort_by...last`.
