PATTERN = T.let(/foo/, Regexp)
          ^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for Regexp literal. Sorbet can infer this type automatically.
