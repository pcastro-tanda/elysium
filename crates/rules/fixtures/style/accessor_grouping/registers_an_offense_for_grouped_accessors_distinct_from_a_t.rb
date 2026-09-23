class Foo
  extend T::Sig

  sig { returns(Integer) }
  attr_reader :one

  attr_reader :two, :three
  ^^^^^^^^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.

  attr_reader :four
  ^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
end
