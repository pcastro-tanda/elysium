class Test
  module_function def foo; end
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.

  module_function
end
