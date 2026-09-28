class Test
  def foo; end
  protected :foo
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
