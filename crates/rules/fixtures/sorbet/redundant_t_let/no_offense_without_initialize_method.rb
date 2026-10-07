sig { params(a: Integer).void }
def different_method(a)
  @a = T.let(a, Integer)
end
