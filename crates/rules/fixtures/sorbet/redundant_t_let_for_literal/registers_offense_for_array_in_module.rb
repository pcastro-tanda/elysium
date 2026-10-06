module Foo
  NAMES = T.let(["a", "b"].freeze, T::Array[String])
          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for Array literal. Sorbet can infer this type automatically.
end
