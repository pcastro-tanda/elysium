class Foo
  foo
  module_function *%i[bar baz]
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
