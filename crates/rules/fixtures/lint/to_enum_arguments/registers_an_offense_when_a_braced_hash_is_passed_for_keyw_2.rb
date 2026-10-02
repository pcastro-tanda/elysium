def m(**kwargs)
  return to_enum(:m, { **kwargs }) unless block_given?
         ^^^^^^^^^^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
