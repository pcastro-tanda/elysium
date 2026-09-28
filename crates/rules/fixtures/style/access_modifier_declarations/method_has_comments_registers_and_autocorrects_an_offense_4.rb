class Test
  # comment
  module_function def foo
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
    # comment
  end
end
