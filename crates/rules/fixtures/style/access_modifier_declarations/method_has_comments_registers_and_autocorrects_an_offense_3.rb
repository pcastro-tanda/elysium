class Test
  # comment
  public def foo
  ^^^^^^ `public` should not be inlined in method definitions.
    # comment
  end
end
