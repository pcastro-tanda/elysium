KEYS = T.let(%I[key_#{a} key_#{b}], T::Array[Symbol])
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for Array literal. Sorbet can infer this type automatically.
