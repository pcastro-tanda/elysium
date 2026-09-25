class SomeClass
  delegate :foo, to: :bar

  private
  ^^^^^^^ Useless `private` access modifier.
end
