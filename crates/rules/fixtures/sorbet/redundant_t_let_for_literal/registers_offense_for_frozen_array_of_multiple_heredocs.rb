MESSAGES = T.let([<<~A, <<~B].freeze, T::Array[String])
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for Array literal. Sorbet can infer this type automatically.
  first
A
  second
B
