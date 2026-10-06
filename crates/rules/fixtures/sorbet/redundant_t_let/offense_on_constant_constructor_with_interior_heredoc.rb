PATH = T.let(
       ^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The constant type is inferred from the constructor.
  Foo.new(
    a: <<~DIR,
      /usr/local
    DIR
    b: 2,
  ),
  Foo,
)
