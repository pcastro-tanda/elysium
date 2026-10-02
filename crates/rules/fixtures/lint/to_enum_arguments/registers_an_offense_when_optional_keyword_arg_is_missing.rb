def m(x, y = 1, *args, required:, optional: true)
  return to_enum(:m, x, y, *args, required: required) unless block_given?
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
