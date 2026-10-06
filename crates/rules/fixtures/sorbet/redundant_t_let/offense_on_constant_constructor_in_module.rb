module Foo
  DEFAULT = T.let(Pathname.new("/x").freeze, Pathname)
            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The constant type is inferred from the constructor.
end
