class Test
  def foo; end
  private :foo
  ^^^^^^^ `private` should not be inlined in method definitions.
end
