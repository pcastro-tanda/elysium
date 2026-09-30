def foo
  render :foo or return if condition
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  do_something
end
