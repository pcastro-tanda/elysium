class A
  def foo=(bar)
    return 42
    ^^^^^^ Do not return a value in `foo=`.
  end
end
