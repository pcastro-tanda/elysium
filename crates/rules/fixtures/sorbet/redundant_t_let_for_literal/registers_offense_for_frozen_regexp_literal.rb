PATTERN = T.let(/foo/.freeze, Regexp)
          ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for Regexp literal. Sorbet can infer this type automatically.
