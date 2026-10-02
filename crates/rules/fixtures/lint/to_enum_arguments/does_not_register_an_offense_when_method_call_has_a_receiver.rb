def m(x)
  return foo.to_enum(:m) unless block_given?
end
