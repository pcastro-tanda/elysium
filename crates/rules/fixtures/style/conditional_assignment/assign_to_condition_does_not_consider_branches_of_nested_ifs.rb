if outer
  bar = 1
else
  if inner
  ^^^^^^^^ Use the return of the conditional for variable assignment and comparison.
    bar = 2
  else
    bar = 3
  end
end
