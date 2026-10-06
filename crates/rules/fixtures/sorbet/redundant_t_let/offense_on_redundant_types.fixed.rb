sig { params(a: Integer, b: String).void }
def initialize(a, b)
  @a = a
  @b = b
end

sig { params(a: Integer, b: String).void }
def initialize(a, b)
  @a = b
end

sig { params(a: Integer).void }
def initialize(a:)
  @a = a
end

sig { params(a: Integer).void }
def initialize(a = 5)
  @a = a
end

sig { params(a: T.nilable(Integer)).void }
def initialize(a)
  @a = a
end

sig { params(a: T::Array[Integer]).void }
def initialize(a)
  @a = a
end

sig { params(a: Foo[Bar], b: T.any(A, B), c: T.proc.void).void }
def initialize(a, b, c)
  @a = T.let(a, Foo[Foo])
  @aa = a
  @b = T.let(b, T.any(B, A))
  @bb = b
  @c = T.let(c, T.proc.returns(Integer))
  @cc = c
end

sig do
  params(
    proc: T.proc.params(a: String).returns(T.nilable(String)),
  ).void
end
def initialize(proc)
  @proc = proc
end

sig { params(a: Integer, b: String, c: String, d: Integer).void }
def initialize(a, b = "hello", c:, d: 1)
  @a = a
  @b = b
  @c = c
  @d = d
end
