class Foo
  OTHER_ATTRS = %i[two three].freeze

  attr_reader :one, *OTHER_ATTRS
end
