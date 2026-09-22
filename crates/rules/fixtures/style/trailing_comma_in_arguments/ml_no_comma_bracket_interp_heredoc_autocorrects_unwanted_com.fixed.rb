some_method(
  bar: <<-BAR,
    #{other_method(a, b)} foo, bar
  BAR
  baz: <<-BAZ
    #{third_method(c, d)} foo, bar
  BAZ
)
