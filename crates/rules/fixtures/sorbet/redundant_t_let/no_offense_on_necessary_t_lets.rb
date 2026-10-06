sig { params(a: Integer) }
def initialize(a)
  @a = T.let(a, String)
end

sig { params(a: Integer).void }
def initialize(a)
  @a = T.let(a, T.any(Integer, String))
end

sig { params(a: Integer).void }
def initialize(a)
  @a = T.let(a.to_s, String)
end

sig { params(a: Integer).void }
def initialize(a)
  number = a
  @answer = T.let(number, Integer)
end

sig { params(a: T.proc.params(x: Integer).returns(String)).void }
def initialize(a)
  @a = T.let(a, T.proc.params(x: String).returns(String))
end
