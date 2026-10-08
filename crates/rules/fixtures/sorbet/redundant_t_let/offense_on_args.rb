sig { params(args: Integer).void }
def initialize(*args)
  @args = T.let(args, T::Array[Integer])
          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end
