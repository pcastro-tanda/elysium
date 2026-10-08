def m(x, &block)
  return to_enum(:m, x) unless block_given?
end
