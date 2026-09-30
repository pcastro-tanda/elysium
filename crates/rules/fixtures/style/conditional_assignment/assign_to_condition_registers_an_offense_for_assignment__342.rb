case foo
^^^^^^^^ Use the return of the conditional for variable assignment and comparison.
when "a"
  bar &&= 1
else
  bar &&= 2
end
