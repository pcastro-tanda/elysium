sig do
  params(
    a: T.any(
      Integer,
      String,
    ),
  ).void
end
def initialize(a)
  @a = a
end
