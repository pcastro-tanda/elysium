case foo
^^^^^^^^ Use the return of the conditional for variable assignment and comparison.
when "a"
  @@cvar ^= 1
else
  @@cvar ^= 2
end
