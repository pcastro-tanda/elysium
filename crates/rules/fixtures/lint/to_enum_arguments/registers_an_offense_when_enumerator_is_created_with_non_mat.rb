def m(...)
  return to_enum(:m, x, ...) unless block_given?
         ^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
