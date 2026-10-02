def m(x:, **kwargs)
  return to_enum(:m, x: x, y: 1) unless block_given?
         ^^^^^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
