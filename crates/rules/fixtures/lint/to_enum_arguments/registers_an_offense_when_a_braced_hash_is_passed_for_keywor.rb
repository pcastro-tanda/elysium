def m(required:)
  return to_enum(:m, { required: required }) unless block_given?
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
