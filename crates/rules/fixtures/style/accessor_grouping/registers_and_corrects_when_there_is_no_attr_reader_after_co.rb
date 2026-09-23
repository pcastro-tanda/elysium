class Foo
  attr_reader :one
  ^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  attr_reader :bar
  ^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.

  OTHER_ATTRS = %i[two three].freeze
end
