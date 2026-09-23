class Foo
  attr_reader :bar1, :bar2, :bar3

  protected
  attr_accessor :quux

  private
  attr_reader :baz1, :baz2, :baz4
  attr_writer :baz3

  public
  other_macro :zoo
end
