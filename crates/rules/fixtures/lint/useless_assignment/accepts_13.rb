def some_method(bar)
  foo = 1
  bar ||= (foo = 2)
  [foo, bar]
end
