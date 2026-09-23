class Foo
  attr_reader :one
  ^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  OTHER_ATTRS = %i[two three].freeze

  attr_reader(*OTHER_ATTRS)
  ^^^^^^^^^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
end
