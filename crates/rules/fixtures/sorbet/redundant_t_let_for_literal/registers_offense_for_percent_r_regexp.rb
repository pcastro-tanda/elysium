PATTERN = T.let(%r{foo/bar}, Regexp)
          ^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for Regexp literal. Sorbet can infer this type automatically.
