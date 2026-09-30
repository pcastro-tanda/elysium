case foo
^^^^^^^^ Use the return of the conditional for variable assignment and comparison.
when foobar
  foo = 1
  bar = 1
when foobaz
  foo = 2
  bar = 2
else
  foo = 3
  bar = 3
end
