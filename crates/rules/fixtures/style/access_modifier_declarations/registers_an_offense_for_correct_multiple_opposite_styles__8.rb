class TestOne
  module_function def foo; end
end

class TestTwo
  module_function
  ^^^^^^^^^^^^^^^ `module_function` should be inlined in method definitions.
  def foo; end
end
