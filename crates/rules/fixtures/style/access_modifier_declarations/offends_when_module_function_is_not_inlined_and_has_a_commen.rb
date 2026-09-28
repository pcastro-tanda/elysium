class Test
  module_function # hey
  ^^^^^^^^^^^^^^^ `module_function` should be inlined in method definitions.
  def foo; end
end
