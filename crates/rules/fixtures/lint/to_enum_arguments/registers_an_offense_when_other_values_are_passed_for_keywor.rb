def m(required:, optional: true)
  return to_enum(:m, required: something_else, optional: optional) unless block_given?
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
