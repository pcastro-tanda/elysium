module Example
  extend T::Sig

  # Session: string
  sig do
    params(
      session: String,
    ).void
  end
  def initialize(
    session:
  )
    @session = session
  end
end
