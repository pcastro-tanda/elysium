def m(x:, **kwargs)
  return to_enum(:m, x: x, y: 1, **kwargs) unless block_given?
end
