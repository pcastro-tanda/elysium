def m(x, y = 1)
  return to_enum(:m, x, y, z) unless block_given?
         ^^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
