some_method(
  bar: <<-BAR,
    #{other_method(a, b,)} foo, bar
                       ^ Avoid comma after the last parameter of a method call.
  BAR
  baz: <<-BAZ
    #{third_method(c, d,)} foo, bar
                       ^ Avoid comma after the last parameter of a method call.
  BAZ
)
