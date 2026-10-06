CONFIG = T.let(Config.new { |c| c.enabled = true }.freeze, Config)
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The constant type is inferred from the constructor.
