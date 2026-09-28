module Foo
  attr_accessor :foo
  attr_writer :bar

  private

  attr_accessor :baz
  attr_reader :bar
end
