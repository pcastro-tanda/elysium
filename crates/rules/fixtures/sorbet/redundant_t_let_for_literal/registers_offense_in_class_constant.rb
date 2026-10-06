class Foo
  MAX = T.let(100, Integer)
        ^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for Integer literal. Sorbet can infer this type automatically.
end
