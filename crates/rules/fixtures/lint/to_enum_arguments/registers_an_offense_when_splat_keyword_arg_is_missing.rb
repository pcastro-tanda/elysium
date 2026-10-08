def m(x, y = 1, *args, required:, optional: true, **kwargs)
  return to_enum(:m, x, y, *args, required: required, optional: optional) unless block_given?
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
