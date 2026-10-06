VALUES = T.let([1, "a", nil].freeze, T::Array[T.nilable(T.any(Integer, String))])
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for Array literal. Sorbet can infer this type automatically.
