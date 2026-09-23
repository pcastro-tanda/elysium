class Foo
  attr_reader :bar1
  attr_reader :bar2

  protected
  attr_accessor :quux

  private
  attr_reader :baz1
  attr_reader :baz2
  attr_writer :baz3
  attr_reader :baz4

  public
  attr_reader :bar3
  other_macro :zoo
end
