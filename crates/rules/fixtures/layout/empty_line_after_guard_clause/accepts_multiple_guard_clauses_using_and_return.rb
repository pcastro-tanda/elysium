def foo
  render :bar and return if condition1?
  render :baz and return if condition2?

  foobar
end
