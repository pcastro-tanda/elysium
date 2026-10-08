sig { params(a: Integer, b: String).void }
def initialize(a, b)
  @a = T.let(a, Integer)
       ^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
  @b = T.let(b, String)
       ^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end

sig { params(a: Integer, b: String).void }
def initialize(a, b)
  @a = T.let(b, String)
       ^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end

sig { params(a: Integer).void }
def initialize(a:)
  @a = T.let(a, Integer)
       ^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end

sig { params(a: Integer).void }
def initialize(a = 5)
  @a = T.let(a, Integer)
       ^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end

sig { params(a: T.nilable(Integer)).void }
def initialize(a)
  @a = T.let(a, T.nilable(Integer))
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end

sig { params(a: T::Array[Integer]).void }
def initialize(a)
  @a = T.let(a, T::Array[Integer])
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end

sig { params(a: Foo[Bar], b: T.any(A, B), c: T.proc.void).void }
def initialize(a, b, c)
  @a = T.let(a, Foo[Foo])
  @aa = T.let(a, Foo[Bar])
        ^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
  @b = T.let(b, T.any(B, A))
  @bb = T.let(b, T.any(A, B))
        ^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
  @c = T.let(c, T.proc.returns(Integer))
  @cc = T.let(c, T.proc.void)
        ^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end

sig do
  params(
    proc: T.proc.params(a: String).returns(T.nilable(String)),
  ).void
end
def initialize(proc)
  @proc = T.let(proc, T.proc.params(a: String).returns(T.nilable(String)))
          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end

sig { params(a: Integer, b: String, c: String, d: Integer).void }
def initialize(a, b = "hello", c:, d: 1)
  @a = T.let(a, Integer)
       ^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
  @b = T.let(b, String)
       ^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
  @c = T.let(c, String)
       ^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
  @d = T.let(d, Integer)
       ^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end
