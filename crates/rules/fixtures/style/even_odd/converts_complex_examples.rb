if (y % 2) != 1
   ^^^^^^^^^^^^ Replace with `Integer#even?`.
  method == :== ? :even : :odd
elsif x % 2 == 1
      ^^^^^^^^^^ Replace with `Integer#odd?`.
  method == :== ? :odd : :even
end
