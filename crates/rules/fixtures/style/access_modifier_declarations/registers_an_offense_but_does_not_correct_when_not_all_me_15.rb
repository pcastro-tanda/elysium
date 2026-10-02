module Foo
  def bar; end

  module_function :bar, :baz
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
