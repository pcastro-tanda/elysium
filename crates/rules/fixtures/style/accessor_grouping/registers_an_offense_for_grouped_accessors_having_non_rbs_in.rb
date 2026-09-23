class Foo
  attr_reader :one # comment #: String
  ^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  attr_reader :two, :three
  ^^^^^^^^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
end
