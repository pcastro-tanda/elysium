class Foo
  extend T::Sig

  annotation_method :one
  attr_reader :one

  annotation_method :two
  attr_reader :two

  sig { returns(Integer) }
  attr_reader :three
end
