sig { params(a: Integer).void }
def initialize(a)
  @a = T.let(a, Integer)
rescue
  @a = T.let(0, Integer)
end
