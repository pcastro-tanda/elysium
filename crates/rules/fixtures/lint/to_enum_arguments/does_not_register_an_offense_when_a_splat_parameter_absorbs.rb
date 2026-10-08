def m(x, *args)
  return to_enum(:m, x, *args) unless block_given?
end
