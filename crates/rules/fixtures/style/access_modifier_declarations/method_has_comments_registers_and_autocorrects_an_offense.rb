class Test
  # comment
  private def foo
  ^^^^^^^ `private` should not be inlined in method definitions.
    # comment
  end
end
