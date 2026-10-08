def m(x)
  return to_enum(__method__) unless block_given?
         ^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
