class Foo
  extend T::Sig

  sig { returns(Integer) }
  attr_reader :one

  attr_reader :two, :three, :four
end
