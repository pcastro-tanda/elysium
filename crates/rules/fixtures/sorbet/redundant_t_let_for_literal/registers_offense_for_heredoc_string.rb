MSG = T.let(<<~MESSAGE, String)
      ^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for String literal. Sorbet can infer this type automatically.
  hello world
MESSAGE
