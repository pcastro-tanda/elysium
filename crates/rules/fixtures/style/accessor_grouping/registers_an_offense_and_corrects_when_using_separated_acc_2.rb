class Foo
  attr_reader :bar1
  ^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  attr_reader :bar2
  ^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.

  protected
  attr_accessor :quux

  private
  attr_reader :baz1, :baz2
  ^^^^^^^^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  attr_writer :baz3
  attr_reader :baz4
  ^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.

  public
  attr_reader :bar3
  ^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  other_macro :zoo
end
