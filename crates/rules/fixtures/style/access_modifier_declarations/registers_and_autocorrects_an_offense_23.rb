class Test
  def foo; end
  module_function :foo
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
