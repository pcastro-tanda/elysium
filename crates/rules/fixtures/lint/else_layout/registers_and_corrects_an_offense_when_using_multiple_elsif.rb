if condition_foo
  foo
elsif condition_bar
  bar
elsif condition_baz
  baz
else qux
     ^^^ Odd `else` layout detected. Did you mean to use `elsif`?
  quux
  corge
end
