def some_method
  _foo = 1
  ^^^^ Do not use prefix `_` for a variable that is used.
  puts _foo
end
