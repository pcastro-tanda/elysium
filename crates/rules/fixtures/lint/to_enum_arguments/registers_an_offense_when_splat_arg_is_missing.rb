def m(x, y = 1, *args)
  return to_enum(:m, x, y) unless block_given?
         ^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
