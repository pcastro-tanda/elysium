def some_method
  foo => { bar: bar }
  baz { bar -= 1 }
  foo
end
