class Test
  private def foo; end
  private def bar; end
  ^^^^^^^ `private` should not be inlined in method definitions.
  def baz; end
  QUX = ['qux']
end
