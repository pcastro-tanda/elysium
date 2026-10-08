PATH = T.let(Pathname.new(<<~DIR), Pathname) # keep me
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The constant type is inferred from the constructor.
  /usr/local
DIR
