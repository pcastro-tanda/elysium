class Foo
  # @return [String] value of foo
  attr_reader :one, :two

  # [String] Some bar value return
  attr_reader :three

  attr_reader :four
  ^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  attr_reader :five
  ^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
end
