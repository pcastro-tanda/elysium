def some_method
  foo => { bar: bar }
  baz { qux -= 1 }
        ^^^ Useless assignment to variable - `qux`. Use `-` instead of `-=`.
  foo
end
