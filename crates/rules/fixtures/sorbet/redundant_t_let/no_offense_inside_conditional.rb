sig { params(a: Integer).void }
def initialize(a)
  if a > 0
    @a = T.let(a, Integer)
  end
end
