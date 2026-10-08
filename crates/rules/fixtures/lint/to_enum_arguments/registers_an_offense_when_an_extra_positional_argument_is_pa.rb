def m(x)
  return to_enum(:m, x, extra) unless block_given?
         ^^^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
