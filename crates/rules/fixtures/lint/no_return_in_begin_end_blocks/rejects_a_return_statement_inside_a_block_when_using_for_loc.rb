some_value = 10

some_value = begin
  return 1 if rand(1..2).odd?
  ^^^^^^^^ Do not `return` in `begin..end` blocks in assignment contexts.
  2
end
