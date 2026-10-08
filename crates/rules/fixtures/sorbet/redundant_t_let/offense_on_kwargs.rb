sig { params(kwargs: String).void }
def initialize(**kwargs)
  @kwargs = T.let(kwargs, T::Hash[Symbol, String])
            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
end
