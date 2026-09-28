class Foo
  foo
  module_function attr_writer :bar
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
