class Test
  def something_else; end

  def a_method_that_is_public; end

  public
  ^^^^^^ `public` should be inlined in method definitions.

  def my_other_private_method; end

  def bar; end
end
