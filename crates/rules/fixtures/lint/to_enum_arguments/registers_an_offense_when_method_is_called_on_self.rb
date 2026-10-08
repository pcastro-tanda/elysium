def m(x)
  return self.to_enum(:m) unless block_given?
         ^^^^^^^^^^^^^^^^ Ensure you correctly provided all the arguments.
end
