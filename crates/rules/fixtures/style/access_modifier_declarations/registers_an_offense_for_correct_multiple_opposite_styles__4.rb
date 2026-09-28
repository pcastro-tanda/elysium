class TestOne
  module_function
end

class TestTwo
  module_function def foo; end
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end

class TestThree
  module_function def foo; end
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
