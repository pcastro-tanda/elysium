module Example
  extend T::Sig

  sig do
    params(
      session: String,
    ).void
  end

^{} Sorbet/EmptyLineAfterSig: Extra empty line or comment detected
  def initialize(
    session:
  )
    @session = session
  end
end
