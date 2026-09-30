def foo
  render :foo and return if condition
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  do_something
end
