def m(...)
  return to_enum(:m, ...) unless block_given?
end
