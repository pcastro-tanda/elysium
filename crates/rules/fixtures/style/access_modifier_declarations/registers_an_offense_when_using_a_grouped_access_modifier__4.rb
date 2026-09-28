class Test
  def something_else; end

  def a_method_that_is_public; end

  module_function
  ^^^^^^^^^^^^^^^ `module_function` should be inlined in method definitions.

  def my_other_private_method; end

  def bar; end
end
