def foo
  render :foo or return if condition

  do_something
end
