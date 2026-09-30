case foo
^^^^^^^^ Use the return of the conditional for variable assignment and comparison.
when "a"
  @ivar <<= 1
else
  @ivar <<= 2
end
