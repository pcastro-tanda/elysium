class Foo
  attr_reader :bar1, :bar2
  ^^^^^^^^^^^^^^^^^^^^^^^^ Use one attribute per `attr_reader`.

  protected
  attr_accessor :quux

  private
  attr_reader :baz1, :baz2
  ^^^^^^^^^^^^^^^^^^^^^^^^ Use one attribute per `attr_reader`.
  attr_writer :baz3
  attr_reader :baz4

  public
  attr_reader :bar3
  other_macro :zoo
end
