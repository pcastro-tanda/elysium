class Foo
  foo
  module_function :bar
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
