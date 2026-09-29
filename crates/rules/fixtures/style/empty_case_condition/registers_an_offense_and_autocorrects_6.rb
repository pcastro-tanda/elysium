case
^^^^ Do not use empty `case` condition, instead use an `if` expression.
when false
  foo
when nil, false, 1
  bar
when false, 1
  baz
end
