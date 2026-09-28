class Test
  module_function attr_writer :foo
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
