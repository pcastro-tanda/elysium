MSG = T.let(
      ^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for String literal. Sorbet can infer this type automatically.
  <<~MESSAGE,
    hello world
  MESSAGE
  String
)
