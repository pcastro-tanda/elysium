module Foo
  class BarError < StandardError
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `Class.new` instead of the `class` keyword to define an empty class.
  end
end
