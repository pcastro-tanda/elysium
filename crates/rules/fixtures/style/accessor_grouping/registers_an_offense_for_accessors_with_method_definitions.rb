class Foo
  def foo
  end
  attr_reader :one
  ^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.

  def bar
  end
  attr_reader :two
  ^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
end
