def some_method
  foo in { bar: bar }
  baz { qux -= 1 }
        ^^^ Useless assignment to variable - `qux`. Use `-` instead of `-=`.
  foo
end
