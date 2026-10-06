ROOT = ::T.let(Pathname.new("/").freeze, Pathname)
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The constant type is inferred from the constructor.
