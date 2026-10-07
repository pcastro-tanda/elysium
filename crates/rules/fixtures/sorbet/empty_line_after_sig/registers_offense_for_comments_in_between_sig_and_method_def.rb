module Example
  extend T::Sig

  sig do
    params(
      session: String,
    ).void
  end
  # Session: string
^^^^^^^^^^^^^^^^^^^ Sorbet/EmptyLineAfterSig: Extra empty line or comment detected
  def initialize(
    session:
  )
    @session = session
  end
end
