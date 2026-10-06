sig { params(a: T.any(Integer, String)).void }
def initialize(a)
  @a = T.let(a, T.any(
       ^^^^^^^^^^^^^^^ Sorbet/RedundantTLet: Unnecessary T.let. The instance variable type is inferred from the signature.
    Integer,
    String
  ))
end
