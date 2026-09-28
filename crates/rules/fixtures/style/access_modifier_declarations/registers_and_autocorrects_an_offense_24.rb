class Test
  module_function attr :foo
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
