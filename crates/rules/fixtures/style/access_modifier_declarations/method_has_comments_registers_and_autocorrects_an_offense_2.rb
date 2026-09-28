class Test
  # comment
  protected def foo
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
    # comment
  end
end
