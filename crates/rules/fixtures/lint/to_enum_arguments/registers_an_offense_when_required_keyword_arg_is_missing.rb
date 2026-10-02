def m(x, y = 1, *args, required:)
  return to_enum(:m, x, y, *args) unless block_given?
         ^^^^^^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
