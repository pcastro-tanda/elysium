sig { params(a: Integer).void }
private def initialize(a)
  @a = T.let(a, Integer)
end
