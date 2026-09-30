if foo
^^^^^^ Use the return of the conditional for variable assignment and comparison.
  @@cvar &&= 1
else
  @@cvar &&= 2
end
