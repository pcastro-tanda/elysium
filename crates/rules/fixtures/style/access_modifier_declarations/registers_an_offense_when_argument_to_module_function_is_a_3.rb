class Foo
  module_function alias_method :bar, :foo
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
