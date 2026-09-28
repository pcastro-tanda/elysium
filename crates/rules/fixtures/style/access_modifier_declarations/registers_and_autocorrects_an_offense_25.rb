class Test
  module_function attr_reader :foo
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
