def m(x)
  return to_enum(:not_m) unless block_given?
end
