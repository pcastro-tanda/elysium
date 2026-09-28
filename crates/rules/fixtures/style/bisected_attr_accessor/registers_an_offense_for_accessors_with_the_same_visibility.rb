class Foo
  attr_reader :foo
              ^^^^ Combine both accessors into `attr_accessor :foo`.

  private
  attr_writer :bar

  public
  attr_writer :foo
              ^^^^ Combine both accessors into `attr_accessor :foo`.
end
