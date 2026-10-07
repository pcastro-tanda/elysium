MSG = T.let(<<~MESSAGE, String) # keep me
      ^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for String literal. Sorbet can infer this type automatically.
  hello world
MESSAGE
